use crate::core::{self, err, Result};
use base64::Engine;
use serde_json::{json, Value};
use std::{io::{Read, Write}, path::PathBuf, process::Stdio,
    sync::{Mutex, atomic::{AtomicBool, Ordering}, mpsc}, time::Duration};

#[repr(C)]
struct Blob { len: u32, data: *mut u8 }
#[link(name="crypt32")]
extern "system" {
    fn CryptProtectData(input:*const Blob, description:*const u16, entropy:*const Blob, reserved:*mut std::ffi::c_void, prompt:*mut std::ffi::c_void, flags:u32, output:*mut Blob)->i32;
    fn CryptUnprotectData(input:*const Blob, description:*mut *mut u16, entropy:*const Blob, reserved:*mut std::ffi::c_void, prompt:*mut std::ffi::c_void, flags:u32, output:*mut Blob)->i32;
}
#[link(name="kernel32")]
extern "system" { fn LocalFree(memory:*mut std::ffi::c_void)->*mut std::ffi::c_void; }
fn crypt(bytes:&[u8], protect:bool)->Result<Vec<u8>> {
    let input=Blob{len:bytes.len() as u32,data:bytes.as_ptr() as *mut u8};
    let mut output=Blob{len:0,data:std::ptr::null_mut()};
    let ok=unsafe {
        if protect {CryptProtectData(&input,std::ptr::null(),std::ptr::null(),std::ptr::null_mut(),std::ptr::null_mut(),1,&mut output)}
        else {CryptUnprotectData(&input,std::ptr::null_mut(),std::ptr::null(),std::ptr::null_mut(),std::ptr::null_mut(),1,&mut output)}
    };
    if ok==0{return Err("The API key is inaccessible. Enter it again using this Windows account.".into());}
    let result=unsafe{std::slice::from_raw_parts(output.data,output.len as usize).to_vec()};
    unsafe{LocalFree(output.data.cast());}
    Ok(result)
}
fn protect(key:&str)->Result<String>{Ok(base64::engine::general_purpose::STANDARD.encode(crypt(key.as_bytes(),true)?))}
fn unprotect(value:&str)->Result<String>{
    String::from_utf8(crypt(&base64::engine::general_purpose::STANDARD.decode(value).map_err(|_|"Invalid key storage")?,false)?).map_err(err)
}
fn endpoint(kind:&str, custom:&str)->Result<String>{
    let base=match kind{"openai"=>"https://api.openai.com/v1","openrouter"=>"https://openrouter.ai/api/v1","compatible"=>custom.trim().trim_end_matches('/'),_=>return Err("Onbekende API-aanbieder".into())};
    let parsed=url::Url::parse(base).map_err(|_|"Enter a valid API base URL")?;
    let loopback=matches!(parsed.host_str(),Some("localhost"|"127.0.0.1"|"[::1]"));
    if !(parsed.scheme()=="https"||(parsed.scheme()=="http"&&loopback))||parsed.host_str().is_none()||
        !parsed.username().is_empty()||parsed.password().is_some()||parsed.query().is_some()||parsed.fragment().is_some()||
        base.chars().any(|c|c.is_control()) {
        return Err("Use HTTPS without a username, password, query, or fragment. HTTP is allowed only on localhost.".into());
    }
    Ok(parsed.as_str().trim_end_matches('/').to_string())
}
fn clean_error(data:&Value, key:&str, status:u16)->String {
    let message=data["error"]["message"].as_str().or_else(||data["error"].as_str()).unwrap_or(match status{
        401|403=>"Controleer je API-sleutel en toegangsrechten.",
        402=>"Onvoldoende API-tegoed bij de aanbieder.",
        429=>"API rate limit reached. Try again later.",
        _=>"The provider returned an invalid response.",
    });
    let message=if key.is_empty(){message.to_owned()}else{message.replace(key,"[sleutel verborgen]")};
    format!("API error ({status}): {}",message.chars().take(500).collect::<String>())
}
pub fn validate_config(data:&Value)->Result<()>{
    let entries=data["providers"].as_array().ok_or("Invalid API configuration")?;
    let mut ids=std::collections::HashSet::new();
    for p in entries{
        let id=p["id"].as_str().ok_or("Provider ID is missing")?;
        if !id.starts_with("api-")||!ids.insert(id)||p["model"].as_str().is_none_or(|s|s.is_empty())||!p["protected_key"].is_string(){return Err("Invalid API configuration".into());}
        crate::custom_models::validate(p["name"].as_str().unwrap_or(""),p["category"].as_str().unwrap_or(""))?;
        endpoint(p["kind"].as_str().unwrap_or(""),p["base_url"].as_str().unwrap_or(""))?;
    }
    Ok(())
}
pub struct Providers {root:PathBuf, settings:Mutex<()>, request:Mutex<()>, cancelled:AtomicBool}
impl Providers {
    pub fn new(root:PathBuf)->Self{Self{root,settings:Mutex::new(()),request:Mutex::new(()),cancelled:AtomicBool::new(false)}}
    fn file(&self)->PathBuf{self.root.join("config/api-providers.json")}
    fn entries(&self)->Result<Vec<Value>>{
        if !self.file().exists(){return Ok(vec![]);}
        let data=core::read_json(&self.file())?;validate_config(&data)?;Ok(data["providers"].as_array().unwrap().clone())
    }
    pub fn list(&self)->Result<Value>{
        let _guard=self.settings.lock().map_err(err)?;
        Ok(json!(self.entries()?.into_iter().map(|mut p|{let has=p["protected_key"].as_str().is_some_and(|k|!k.is_empty());p.as_object_mut().unwrap().remove("protected_key");p["key_saved"]=json!(has);p}).collect::<Vec<_>>()))
    }
    pub fn catalog(&self)->Result<Value>{
        let entries=self.list()?;let mut result=json!({});
        for p in entries.as_array().unwrap(){
            let id=p["id"].as_str().ok_or("Provider ID is missing")?;
            result[id]=json!({"name":p["name"],"type":if p["category"]=="vision"{"vision"}else{"chat"},"category":p["category"],
                "description":format!("{} · {}",p["kind"].as_str().unwrap_or("API"),p["model"].as_str().unwrap_or("")),
                "remote":true,"provider":p["kind"],"installed":true,"online":false,"status":"api","bytes":0,
                "custom":true,"can_download":false,"tokens_used":0,"tokens_limit":0});
        }
        Ok(result)
    }
    fn prepare(&self,data:&Value, require_model:bool)->Result<(Value,String)>{
        let kind=data["kind"].as_str().unwrap_or("");
        let base=endpoint(kind,data["base_url"].as_str().unwrap_or(""))?;
        let old=if let Some(id)=data["id"].as_str().filter(|s|!s.is_empty()){
            Some(self.entries()?.into_iter().find(|p|p["id"]==id).ok_or("This API connection no longer exists")?)
        }else{None};
        let name=data["name"].as_str().unwrap_or("").trim();
        let model=data["model"].as_str().unwrap_or("").trim();
        let category=data["category"].as_str().unwrap_or("chat");
        if require_model {
            crate::custom_models::validate(name,category)?;
            if model.is_empty()||model.len()>200||model.chars().any(|c|c.is_control()){return Err("Enter a valid provider model ID".into());}
        }
        let supplied=data["api_key"].as_str().unwrap_or("").trim();
        if supplied.len()>4096||supplied.chars().any(|c|c.is_control()){return Err("Invalid API key".into());}
        let key=if !supplied.is_empty(){supplied.to_string()}else if let Some(ref p)=old{
            if p["base_url"]!=base||p["kind"]!=kind{return Err("Enter the API key again when changing the provider or URL.".into());}
            p["protected_key"].as_str().filter(|k|!k.is_empty()).map(unprotect).transpose()?.unwrap_or_default()
        }else{String::new()};
        if key.is_empty()&&kind!="compatible"{return Err("Vul je API-sleutel in".into());}
        let id=old.as_ref().and_then(|p|p["id"].as_str()).map(str::to_string).unwrap_or_else(||format!("api-{}",core::id()));
        Ok((json!({"id":id,"kind":kind,"base_url":base,"name":name,"model":model,"category":category}),key))
    }
    pub fn save(&self,data:&Value)->Result<Value>{
        let _guard=self.settings.lock().map_err(err)?;
        let (mut entry,key)=self.prepare(data,true)?;
        entry["protected_key"]=json!(if key.is_empty(){String::new()}else{protect(&key)?});
        let id=entry["id"].clone();let mut entries=self.entries()?;
        if let Some(old)=entries.iter_mut().find(|p|p["id"]==id){*old=entry;}else{entries.push(entry);}
        core::atomic(&self.file(),&json!({"version":1,"providers":entries}))?;
        Ok(json!({"id":id}))
    }
    pub fn remove(&self,data:&Value)->Result<Value>{
        let _guard=self.settings.lock().map_err(err)?;
        let id=data["id"].as_str().ok_or("Provider ID is missing")?;
        let mut entries=self.entries()?;entries.retain(|p|p["id"]!=id);
        core::atomic(&self.file(),&json!({"version":1,"providers":entries}))?;Ok(json!({"ok":true}))
    }
    pub fn cancel(&self){self.cancelled.store(true,Ordering::SeqCst);}
    pub fn models(&self,data:&Value)->Result<Value>{
        let _guard=self.request.try_lock().map_err(|_|"An API request is already running")?;
        self.cancelled.store(false,Ordering::SeqCst);
        let (entry,key)={let _settings=self.settings.lock().map_err(err)?;self.prepare(data,false)?};
        let result=self.transport(&entry,&key,"models",None,false,|_|Ok(()))?;
        let models=result["data"].as_array().ok_or("This API does not support listing models; enter the model ID manually.")?;
        let mut ids:Vec<&str>=models.iter().filter_map(|m|m["id"].as_str()).collect();ids.sort_unstable();ids.dedup();Ok(json!({"models":ids}))
    }
    pub fn chat(&self,id:&str,body:Value,stream:bool,emit:impl FnMut(String)->Result<()>)->Result<Value>{
        let _guard=self.request.try_lock().map_err(|_|"An API request is already running")?;
        self.cancelled.store(false,Ordering::SeqCst);
        let entry={let _settings=self.settings.lock().map_err(err)?;self.entries()?.into_iter().find(|p|p["id"]==id).ok_or("This API model is no longer available; choose another model")?};
        // Validate persisted endpoints too, before sending credentials.
        let mut entry=entry;entry["base_url"]=json!(endpoint(entry["kind"].as_str().unwrap_or(""),entry["base_url"].as_str().unwrap_or(""))?);
        let key=entry["protected_key"].as_str().filter(|s|!s.is_empty()).map(unprotect).transpose()?.unwrap_or_default();
        let messages=body["messages"].as_array().ok_or("Messages are missing")?;
        let mut request=json!({"model":entry["model"],"messages":messages,"stream":stream});
        if let Some(limit)=body["max_tokens"].as_u64(){
            request[if entry["kind"]=="openai"{"max_completion_tokens"}else{"max_tokens"}]=json!(limit.clamp(1,32768));
        }
        if entry["kind"]=="openai"{request["store"]=json!(false);}
        if stream{request["stream_options"]=json!({"include_usage":true});}
        self.transport(&entry,&key,"chat/completions",Some(request),stream,emit)
    }
    fn transport(&self,entry:&Value,key:&str,path:&str,body:Option<Value>,stream:bool,mut emit:impl FnMut(String)->Result<()>)->Result<Value>{
        let url=format!("{}/{}",entry["base_url"].as_str().ok_or("API URL is missing")?,path);
        // Config arrives over stdin: secrets and chat contents never enter command arguments or temp files.
        let mut config=format!("url = {}\nheader = \"Content-Type: application/json\"\n",serde_json::to_string(&url).map_err(err)?);
        if !key.is_empty(){config.push_str(&format!("header = {}\n",serde_json::to_string(&format!("Authorization: Bearer {key}")).map_err(err)?));}
        if let Some(body)=body{config.push_str(&format!("data-binary = {}\n",serde_json::to_string(&body.to_string()).map_err(err)?));}
        let mut child=core::hidden("curl.exe").args(["-q","--silent","--no-buffer","--include","--suppress-connect-headers","--globoff","--connect-timeout","15","--max-time","300","--config","-"])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(err)?;
        let stdout=child.stdout.take().ok_or("API output is missing")?;
        let (tx,rx)=mpsc::sync_channel(8);
        let reader=std::thread::spawn(move||{
            let mut stdout=stdout;let mut buffer=[0;8192];
            loop{match stdout.read(&mut buffer){Ok(0)=>break,Ok(n)=>if tx.send(Ok(buffer[..n].to_vec())).is_err(){break;},Err(_)=>{let _=tx.send(Err("API-verbinding onderbroken".to_string()));break;}}}
        });
        let result=(||->Result<Value>{
            child.stdin.take().ok_or("API input is missing")?.write_all(config.as_bytes()).map_err(err)?;
            let mut pending=Vec::new();let mut raw=Vec::new();let mut headers=true;let mut status=0u16;
            let mut content=String::new();let mut usage=Value::Null;let mut total=0usize;let mut done=false;
            loop{
                if self.cancelled.load(Ordering::SeqCst){return Err("API request stopped".into());}
                match rx.recv_timeout(Duration::from_millis(100)){
                    Ok(bytes)=>{let bytes=bytes?;total+=bytes.len();if total>40_000_000{return Err("API-answer te groot".into());}pending.extend(bytes);}
                    Err(mpsc::RecvTimeoutError::Timeout)=>continue,
                    Err(mpsc::RecvTimeoutError::Disconnected)=>break,
                }
                while let Some(end)=pending.iter().position(|b|*b==b'\n'){
                    let line:Vec<u8>=pending.drain(..=end).collect();
                    if headers{
                        let line=String::from_utf8_lossy(&line);
                        if line.starts_with("HTTP/"){status=line.split_whitespace().nth(1).and_then(|s|s.parse().ok()).unwrap_or(0);}
                        if line.trim().is_empty(){headers=status<200;}
                        continue;
                    }
                    if stream&&(200..300).contains(&status){
                        let line=std::str::from_utf8(&line).map_err(|_|"Invalid UTF-8 in API stream")?;
                        if let Some(data)=line.trim_end().strip_prefix("data:"){
                            let data=data.trim();if data=="[DONE]"{done=true;break;}
                            if data.is_empty(){continue;}
                            let chunk:Value=serde_json::from_str(data).map_err(|_|"Invalid API stream")?;
                            if !chunk["error"].is_null(){return Err(clean_error(&chunk,key,status));}
                            if let Some(text)=chunk["choices"][0]["delta"]["content"].as_str(){content.push_str(text);}
                            if !chunk["usage"].is_null(){usage=chunk["usage"].clone();}
                            emit(format!("data: {}\n\n",chunk))?;
                        }
                    }else{raw.extend(line);}
                }
                if done{break;}
                if pending.len()>4_000_000{return Err("API-streamregel te groot".into());}
            }
            if !(200..300).contains(&status){
                raw.extend(pending);
                return Err(clean_error(&serde_json::from_slice(&raw).unwrap_or(Value::Null),key,status));
            }
            if stream{
                if !done{return Err("API-stream onderbroken; answer mogelijk onvolledig.".into());}
                Ok(json!({"choices":[{"message":{"content":content}}],"usage":usage}))
            }else{
                raw.extend(pending);
                let value:Value=serde_json::from_slice(&raw).map_err(|_|"The provider returned an invalid JSON response")?;
                if !value["error"].is_null(){return Err(clean_error(&value,key,status));}Ok(value)
            }
        })();
        // Also terminate on DONE: a provider may keep its HTTP connection open.
        let _=child.kill();let _=child.wait();drop(rx);let _=reader.join();result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn fixture()->(PathBuf,Providers){
        let root=core::root().unwrap().join("tests").join(format!("providers-{}",core::id()));
        fs::create_dir_all(&root).unwrap();let providers=Providers::new(root.clone());(root,providers)
    }
    fn draft(base:&str)->Value{json!({"kind":"compatible","base_url":base,"name":"Test API","model":"test/model","category":"chat","api_key":"test-secret"})}
    #[test]fn api_configuration_survives_backup_and_restore(){
        let (root,p)=fixture();
        let id=p.save(&draft("https://example.com/v1")).unwrap()["id"].clone();
        let cfg=json!({"_root":root,"paths":{"data":root.join("data"),"backups":root.join("backups"),"models":root.join("models"),"logs":root.join("logs")},"services":{}});
        let backup=core::backup(&cfg).unwrap();p.remove(&json!({"id":id})).unwrap();
        core::restore(&cfg,&root,&backup).unwrap();
        assert_eq!(p.list().unwrap()[0]["id"],id);
        assert_eq!(unprotect(p.entries().unwrap()[0]["protected_key"].as_str().unwrap()).unwrap(),"test-secret");
        assert!(validate_config(&json!({"providers":[{"id":"invalid"}]})).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]fn credentials_are_encrypted_and_never_listed(){
        let (root,p)=fixture();let mut data=draft("https://example.com/v1");
        let id=p.save(&data).unwrap()["id"].clone();
        let disk=fs::read_to_string(p.file()).unwrap();assert!(!disk.contains("test-secret"));
        let public=p.list().unwrap();assert_eq!(public[0]["key_saved"],true);assert!(public[0].get("protected_key").is_none());assert!(!public.to_string().contains("test-secret"));
        assert_eq!(p.catalog().unwrap()[id.as_str().unwrap()]["remote"],true);
        data["id"]=id.clone();data["api_key"]=json!("");data["name"]=json!("Renamed");p.save(&data).unwrap();
        assert_eq!(unprotect(p.entries().unwrap()[0]["protected_key"].as_str().unwrap()).unwrap(),"test-secret");
        data["base_url"]=json!("https://other.example/v1");assert!(p.save(&data).is_err());
        data["api_key"]=json!("new-secret");p.save(&data).unwrap();
        assert_eq!(unprotect(p.entries().unwrap()[0]["protected_key"].as_str().unwrap()).unwrap(),"new-secret");
        p.remove(&json!({"id":id})).unwrap();assert_eq!(p.list().unwrap(),json!([]));fs::remove_dir_all(root).unwrap();
    }
    #[test]fn endpoints_reject_credentials_and_insecure_remote_urls(){
        for value in ["http://example.com/v1","file:///test","https://key@example.com/v1","https://example.com/v1?key=secret","https://example.com/v1#fragment"]{assert!(endpoint("compatible",value).is_err(),"{value}");}
        assert!(endpoint("compatible","http://127.0.0.1:1234/v1").is_ok());
        assert_eq!(endpoint("openai","https://wrong.example").unwrap(),"https://api.openai.com/v1");
        assert_eq!(endpoint("openrouter","").unwrap(),"https://openrouter.ai/api/v1");
    }
    #[test]fn api_stream_and_json_preserve_model_messages_and_usage(){
        for stream in [true,false]{
            let (root,p)=fixture();let server=tiny_http::Server::http("127.0.0.1:0").unwrap();
            let base=format!("http://{}/v1",server.server_addr());let id=p.save(&draft(&base)).unwrap()["id"].as_str().unwrap().to_string();
            let thread=std::thread::spawn(move||{
                let mut request=server.recv().unwrap();assert_eq!(request.url(),"/v1/chat/completions");
                assert!(request.headers().iter().any(|h|h.field.equiv("Authorization")&&h.value.as_str()=="Bearer test-secret"));
                let mut text=String::new();request.as_reader().read_to_string(&mut text).unwrap();let body:Value=serde_json::from_str(&text).unwrap();
                assert_eq!(body["model"],"test/model");assert_eq!(body["messages"][0]["content"],"hé 😀");assert_eq!(body["stream"],stream);assert_eq!(body["max_tokens"],42);
                let response=if stream{": keepalive\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"hé \"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"😀\"}}],\"usage\":{\"total_tokens\":7}}\n\ndata: [DONE]\n\n"}else{r#"{"choices":[{"message":{"content":"hé 😀"}}],"usage":{"total_tokens":7}}"#};
                request.respond(tiny_http::Response::from_string(response)).unwrap();
            });
            let mut events=vec![];let answer=p.chat(&id,json!({"messages":[{"role":"user","content":"hé 😀"}],"max_tokens":42}),stream,|e|{events.push(e);Ok(())}).unwrap();
            thread.join().unwrap();assert_eq!(answer["choices"][0]["message"]["content"],"hé 😀");assert_eq!(answer["usage"]["total_tokens"],7);
            assert_eq!(events.len(),if stream{2}else{0});fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]fn api_models_and_errors_are_handled_without_exposing_keys(){
        for status in [200,401,429] {
            let (root,p)=fixture();let server=tiny_http::Server::http("127.0.0.1:0").unwrap();
            let base=format!("http://{}/v1",server.server_addr());
            let thread=std::thread::spawn(move||{
                let request=server.recv().unwrap();assert_eq!(request.url(),"/v1/models");assert_eq!(request.method().as_str(),"GET");
                let body=if status==200{r#"{"data":[{"id":"z"},{"id":"a"},{"id":"a"}]}"#}else{r#"{"error":{"message":"Rejected test-secret"}}"#};
                request.respond(tiny_http::Response::from_string(body).with_status_code(status)).unwrap();
            });
            let result=p.models(&draft(&base));thread.join().unwrap();
            if status==200{assert_eq!(result.unwrap()["models"],json!(["a","z"]));}
            else{let error=result.unwrap_err();assert!(error.contains(&status.to_string()));assert!(!error.contains("test-secret"));}
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]fn api_truncated_stream_is_rejected(){
        let (root,p)=fixture();let server=tiny_http::Server::http("127.0.0.1:0").unwrap();
        let id=p.save(&draft(&format!("http://{}/v1",server.server_addr()))).unwrap()["id"].as_str().unwrap().to_string();
        let thread=std::thread::spawn(move||{let request=server.recv().unwrap();request.respond(tiny_http::Response::from_string("data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n")).unwrap();});
        assert!(p.chat(&id,json!({"messages":[]}),true,|_|Ok(())).unwrap_err().contains("onderbroken"));thread.join().unwrap();fs::remove_dir_all(root).unwrap();
    }
    #[test]fn cancelling_api_request_stops_waiting_for_provider(){
        let (root,p)=fixture();let p=std::sync::Arc::new(p);let server=tiny_http::Server::http("127.0.0.1:0").unwrap();
        let id=p.save(&draft(&format!("http://{}/v1",server.server_addr()))).unwrap()["id"].as_str().unwrap().to_string();
        let client=p.clone();let thread=std::thread::spawn(move||client.chat(&id,json!({"messages":[]}),true,|_|Ok(())));
        let request=server.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
        let start=std::time::Instant::now();p.cancel();let error=thread.join().unwrap().unwrap_err();
        assert!(error.contains("stopped"));assert!(start.elapsed()<Duration::from_secs(3));drop(request);fs::remove_dir_all(root).unwrap();
    }
}