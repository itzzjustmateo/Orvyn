use crate::core::{self, err, Result};
use serde_json::{json, Value};
use std::{collections::HashMap, fs, path::{Path, PathBuf}, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};

const CHAT_INSTRUCTIONS:&str = "You are a helpful assistant. Respond in clear English by default unless the user chooses another language. Answer directly and concretely. Understand ordinary typos, missing spaces, and informal spelling from context without asking the user to correct them. Ask for clarification only when meaning is genuinely unclear or essential information is missing. For simple factual questions, give the fact, date, or period first, then a brief explanation if needed. Do not invent facts; state uncertainty when you are uncertain.";

fn prepare_profile(body:&mut Value,entry:Option<&Value>)->String {
    if let Some(profile)=entry {
        if let Some(limit)=profile["generation"]["max_tokens"].as_u64().filter(|n|*n>0&&*n<=4096){
            body["max_tokens"]=json!(body["max_tokens"].as_u64().filter(|n|*n>0).unwrap_or(limit).min(limit));
        }
        for field in ["temperature","top_p"] {
            if body[field].is_null() {
                if let Some(value)=profile["generation"][field].as_f64().filter(|v|v.is_finite()&&*v>=0.0&&*v<=if field=="top_p"{1.0}else{2.0}){body[field]=json!(value);}
            }
        }
    }
    entry.and_then(|m|m["system_prompt"].as_str()).filter(|s|!s.trim().is_empty()).unwrap_or(CHAT_INSTRUCTIONS).to_string()
}
pub fn catalog(root:&Path)->Result<Vec<Value>> {
    let mut models=vec![];
    for name in ["model-catalog.json","custom-models.json","store-models.json"]{
        let file=root.join("config").join(name);if !file.exists(){continue;}
        models.extend(core::read_json(&file)?["models"].as_array().cloned().ok_or("Invalid model catalog")?);
    }
    Ok(models)
}
pub fn extend_config(root:&Path,cfg:&mut Value)->Result<()> {
    let runtime=cfg["paths"]["llamafile"].as_str().unwrap_or("").to_string();
    let folder=cfg["paths"]["models"].as_str().unwrap_or("models").to_string();
    for (index,m) in catalog(root)?.iter().enumerate(){
        let id=m["id"].as_str().ok_or("Model ID is missing")?;
        let file=m["file"].as_str().ok_or("Model file is missing")?;
        if !file.ends_with(".gguf")||!file.bytes().all(|b|b.is_ascii_alphanumeric()||b"._-".contains(&b)){return Err("Invalid filename in catalog".into());}
        if !cfg["services"][id].is_null(){continue;}
        let port=8082+index as u16;
        cfg["services"][id]=json!({"name":m["name"],"category":m["category"],"port":port,"command":runtime,"args":["--server","--model",format!("{folder}/{file}"),"--host","127.0.0.1","--port",port.to_string(),"--ctx-size","4096","--n-predict","4096","--gpu","disable","--nologo"]});
        if m["runtime_profile"]=="single"{cfg["services"][id]["args"].as_array_mut().unwrap().extend([json!("--parallel"),json!("1")]);}
        if let Some(projector)=m["projector"]["file"].as_str(){
            if !projector.ends_with(".gguf")||!projector.bytes().all(|b|b.is_ascii_alphanumeric()||b"._-".contains(&b)){return Err("Invalid projector filename".into());}
            cfg["services"][id]["args"].as_array_mut().unwrap().extend([json!("--mmproj"),json!(format!("{folder}/{projector}"))]);
        }
    }
    Ok(())
}
fn model_path(cfg:&Value,key:&str)->Option<PathBuf>{
    let args=cfg["services"][key]["args"].as_array()?;
    let index=args.iter().position(|v|v=="--model")?;
    args.get(index+1)?.as_str().map(PathBuf::from)
}
pub(crate) fn downloaded_file_valid(path:&Path,metadata:&Value)->bool{
    fs::metadata(path).is_ok_and(|v|metadata["bytes"].as_u64().unwrap_or(0)==0||Some(v.len())==metadata["bytes"].as_u64()) && core::read_json(&path.with_extension("gguf.verified.json")).is_ok_and(|v|v["sha256"]==metadata["sha256"])
}
fn is_installed(cfg:&Value,key:&str,entry:Option<&Value>)->bool {
    let Some(file)=model_path(cfg,key) else{return false};
    if !file.is_file(){return false;}
    match entry {
        None=>true,
        Some(m)=>{
            downloaded_file_valid(&file,m)&&m["projector"]["file"].as_str().is_none_or(|name|downloaded_file_valid(&core::path(cfg,"models").join(name),&m["projector"]))
        },
    }
}
struct State { active:Option<String>, phase:String, last_use:Instant, tokens:HashMap<String,u64> }
pub struct Runtime {
    root:PathBuf,
    pub providers:crate::providers::Providers,
    inference:Mutex<()>,
    state:Mutex<State>,
    download:Mutex<Value>,
    cancel:Arc<AtomicBool>,
    cancel_generation:AtomicBool,
    closing:AtomicBool,
    idle_seconds:u64,
    selected_files:Mutex<HashMap<String,PathBuf>>,
    queue:Mutex<Vec<String>>,
    queue_paused:AtomicBool,
}
impl Runtime {
    pub fn with_config(root:PathBuf,cfg:&Value)->Result<Arc<Self>> {
        let idle_seconds=cfg["model_policy"]["idle_seconds"].as_u64().unwrap_or(1800).max(1);
        let runtime=Arc::new(Self{providers:crate::providers::Providers::new(root.clone()),root,inference:Mutex::new(()),state:Mutex::new(State{active:None,phase:"idle".into(),last_use:Instant::now(),tokens:HashMap::new()}),download:Mutex::new(json!({"status":"idle"})),cancel:Arc::new(AtomicBool::new(false)),cancel_generation:AtomicBool::new(false),closing:AtomicBool::new(false),idle_seconds,selected_files:Mutex::new(HashMap::new()),queue:Mutex::new(vec![]),queue_paused:AtomicBool::new(false)});
        // Delay orphan cleanup; the first chat also stops previous owned processes.

        let weak=Arc::downgrade(&runtime);
        let startup=Instant::now();let mut cleanup_done=false;
        std::thread::spawn(move||loop{
            std::thread::sleep(Duration::from_secs(2));
            let Some(runtime)=weak.upgrade()else{break};
            if runtime.closing.load(Ordering::Relaxed){break;}
            runtime.advance_queue();
            if let Ok(_guard)=runtime.inference.try_lock(){
                if !cleanup_done&&startup.elapsed()>=Duration::from_secs(10){
                    if runtime.state.lock().map(|s|s.active.is_none()).unwrap_or(false){let _=runtime.stop_all();}
                    cleanup_done=true;
                }
                let due=runtime.state.lock().map(|s|idle_due(s.active.is_some(),s.last_use.elapsed(),runtime.idle_seconds)).unwrap_or(false);
                if due {if runtime.stop_all().is_ok(){if let Ok(mut state)=runtime.state.lock(){state.active=None;state.phase="idle".into();}}}
            };
        });
        Ok(runtime)
    }
    fn stop_all(&self)->Result<()> {
        let cfg=core::config(&self.root)?;
        for key in cfg["services"].as_object().unwrap().keys(){
            // core::service only terminates matching, recorded process identities.
            if core::service_owned(&cfg,key){core::service(&self.root,key,false)?;}
        }
        Ok(())
    }
    pub fn shutdown(&self){self.providers.cancel();self.closing.store(true,Ordering::Relaxed);self.cancel.store(true,Ordering::Relaxed);let _=self.stop_all();}
    pub fn cancel_answer(&self)->Result<()> {
        self.providers.cancel();
        self.cancel_generation.store(true,Ordering::Relaxed);
        let active=self.state.lock().map_err(err)?.active.clone();
        if let Some(key)=active{core::service(&self.root,&key,false)?;}
        Ok(())
    }
    pub fn unload(&self)->Result<String>{
        let _guard=self.inference.try_lock().map_err(|_|"A response is still being generated")?;
        self.stop_all()?;let mut state=self.state.lock().map_err(err)?;state.active=None;state.phase="idle".into();Ok("Models unloaded".into())
    }
    pub fn list(&self)->Result<Value>{
        let cfg=core::config(&self.root)?;let entries=catalog(&self.root)?;
        let state=self.state.lock().map_err(err)?;
        let mut result=json!({});
        for (key,svc) in cfg["services"].as_object().unwrap(){
            let entry=entries.iter().find(|m|m["id"]==key.as_str());
            let installed=is_installed(&cfg,key,entry);let online=installed&&core::service_owned(&cfg,key)&&core::online(&cfg,key);
            let category=entry.and_then(|m|m["category"].as_str()).unwrap_or(if key=="glm-ocr"{"vision"}else{"reasoning"});
            let status=if state.active.as_ref()==Some(key){state.phase.as_str()}else if online{"loaded"}else if installed{"unloaded"}else{"not_installed"};
            result[key]=json!({"name":svc["name"],"type":if category=="vision"{"vision"}else{"chat"},"category":category,"description":entry.and_then(|m|m["description"].as_str()).unwrap_or(if key=="glm-ocr"{"Tekstherkenning uit afbeeldingen"}else{"Qwen3 Thinking for reasoning and chat"}),"installed":installed,"online":online,"status":status,"bytes":entry.and_then(|m|m["bytes"].as_u64()).filter(|b|*b>0).or_else(||model_path(&cfg,key).and_then(|p|fs::metadata(p).ok().map(|m|m.len()))).unwrap_or(0),"custom":entry.is_some_and(|m|m["origin"]=="custom"||m["origin"]=="local"),"store":entry.is_some_and(|m|m["origin"]=="store"),"downloads":entry.and_then(|m|m["downloads"].as_u64()).unwrap_or(0),"can_download":entry.is_some_and(|m|m["url"].is_string()),"tokens_used":state.tokens.get(key).copied().unwrap_or(0),"tokens_limit":2000000});
            if let Some(bytes)=entry.and_then(|m|m["projector"]["bytes"].as_u64()){result[key]["bytes"]=json!(result[key]["bytes"].as_u64().unwrap_or(0)+bytes);}
        }
        result.as_object_mut().unwrap().extend(self.providers.catalog()?.as_object().unwrap().clone());
        Ok(result)
    }
    pub fn overview(&self)->Result<Value>{
        let models=self.list()?;
        let state=self.state.lock().map_err(err)?;
        let mut download=self.download.lock().map_err(err)?.clone();
        if let Some(id)=download["id"].as_str(){
            let cfg=core::config(&self.root)?;
            if let Some(file)=download["file"].as_str().map(|f|core::path(&cfg,"models").join(f)).or_else(||model_path(&cfg,id)){
                let path=if download["status"]=="ready"{file}else{file.with_extension("gguf.part")};
                download["bytes"]=json!(fs::metadata(path).map(|m|m.len()).unwrap_or(0));
            }
        }
        let selection:HashMap<String,String>=self.selected_files.lock().map_err(err)?.iter().map(|(slot,path)|(slot.clone(),path.file_name().unwrap_or_default().to_string_lossy().into_owned())).collect();
        Ok(json!({"models":models,"runtime":{"active":state.active,"phase":state.phase,"idle_seconds":self.idle_seconds,"idle_remaining":self.idle_seconds.saturating_sub(state.last_use.elapsed().as_secs())},"download":download,"selection":selection,"queue":self.queue.lock().map_err(err)?.clone(),"queue_paused":self.queue_paused.load(Ordering::Relaxed)}))
    }
    pub fn chat(&self,key:&str,body:Value)->Result<Value>{
        if key.starts_with("api-"){return self.providers.chat(key,body,false,|_|Ok(()));}
        self.chat_with(key,body,core::chat_response)
    }
    pub fn stream(&self,key:&str,body:Value,mut emit:impl FnMut(String)->Result<()>)->Result<Value>{
        if key.starts_with("api-"){return self.providers.chat(key,body,true,emit);}
        self.chat_with(key,body,|cfg,key,body|core::stream_response(cfg,key,body,&mut emit))
    }
    pub fn chat_with(&self,key:&str,mut body:Value,respond:impl FnOnce(&Value,&str,Value)->Result<Value>)->Result<Value>{
        let _guard=self.inference.try_lock().map_err(|_|"A model request is already running; wait for it to finish")?;
        if self.closing.load(Ordering::Relaxed){return Err("App sluit af".into());}
        self.cancel_generation.store(false,Ordering::Relaxed);
        let cfg=core::config(&self.root)?;let entries=catalog(&self.root)?;
        let entry=entries.iter().find(|m|m["id"]==key);
        if !is_installed(&cfg,key,entry){return Err("Download this model from the AI library first".into());}
        let previous=self.state.lock().map_err(err)?.active.clone();
        let ready=previous.as_deref()==Some(key)&&core::service_owned(&cfg,key)&&core::online(&cfg,key);
        if previous.as_deref()!=Some(key){self.stop_all()?;}
        {let mut state=self.state.lock().map_err(err)?;state.active=Some(key.into());state.phase="loading".into();}
        let result=(||{
            if !ready{core::service(&self.root,key,true)?;}
            let mut healthy=ready;
            for _ in 0..120{
                if healthy{break;}
                if self.closing.load(Ordering::Relaxed)||self.cancel_generation.load(Ordering::Relaxed){return Err("Request stopped".into());}
                if !core::service_owned(&cfg,key){return Err("Model failed to load. Check the service log for compatibility issues or missing files.".into());}
                let url=format!("http://127.0.0.1:{}/health",cfg["services"][key]["port"]);
                let output=core::hidden("curl.exe").args(["-sS","--noproxy","*","--max-time","2",&url]).output().map_err(err)?;
                if serde_json::from_slice::<Value>(&output.stdout).ok().is_some_and(|v|v["status"]=="ok"){healthy=true;break;}
                std::thread::sleep(Duration::from_secs(1));
            }
            if !healthy{return Err("Model did not become ready; check the service log".into());}
            self.state.lock().map_err(err)?.phase="generating".into();
            let instructions=prepare_profile(&mut body,entry);
            if key!="glm-ocr"&&!entry.is_some_and(|m|m["category"]=="vision"){if let Some(messages)=body["messages"].as_array_mut(){
                if entry.is_some_and(|m|m["system_role"]==false){
                    if let Some(first)=messages.iter_mut().find(|m|m["role"]=="user") {if let Some(text)=first["content"].as_str(){first["content"]=json!(format!("{instructions}\n\n{text}"));}}
                }else{messages.insert(0,json!({"role":"system","content":instructions}));}
            }}
            respond(&cfg,key,body)
        })();
        let result=if self.cancel_generation.load(Ordering::Relaxed){Err("Request stopped".into())}else{result};
        if result.is_err(){let _=core::service(&self.root,key,false);}
        let mut state=self.state.lock().map_err(err)?;state.last_use=Instant::now();
        if let Ok(ref response)=result {state.phase="loaded".into();*state.tokens.entry(key.into()).or_default()+=response["usage"]["total_tokens"].as_u64().unwrap_or(0);}
        else {state.active=None;state.phase="idle".into();}
        result
    }
    pub fn start_download(self:&Arc<Self>,key:&str)->Result<()> {
        let cfg=core::config(&self.root)?;
        let entry=catalog(&self.root)?.into_iter().find(|m|m["id"]==key).ok_or("No download is available for this model")?;
        if is_installed(&cfg,key,Some(&entry)){return Ok(());}
        if !entry["url"].is_string(){return Err("This local file is missing; import it again".into());}
        {let mut job=self.download.lock().map_err(err)?;
            if job["status"]=="downloading"||job["status"]=="verifying"||job["status"]=="importing"{return Err("A download or import is already running".into());}
            *job=json!({"id":key,"status":"downloading","total":entry["bytes"],"bytes":0,"message":"Starting download"});
        }
        self.cancel.store(false,Ordering::Relaxed);
        let runtime=self.clone();std::thread::spawn(move||{
            let result=(||->Result<String>{
                let mut parts=vec![&entry];if entry["projector"]["url"].is_string(){parts.push(&entry["projector"]);}
                for part in parts{
                    if runtime.cancel.load(Ordering::Relaxed){return Ok("Download paused".into());}
                    if let Some(file)=part["file"].as_str(){if downloaded_file_valid(&core::path(&cfg,"models").join(file),part){continue;}}
                    if let Ok(mut job)=runtime.download.lock(){job["file"]=part["file"].clone();job["total"]=part["bytes"].clone();job["status"]=json!("downloading");}
                    core::download(&cfg,part["url"].as_str().ok_or("Download link is missing")?,part["file"].as_str().ok_or("Filename is missing")?,part["sha256"].as_str().ok_or("Checksum is missing")?,runtime.cancel.clone(),|text|{
                        if let Ok(mut job)=runtime.download.lock(){if text.contains("SHA-256"){job["status"]=json!("verifying");}job["message"]=json!(text);}
                    })?;
                }Ok("Model en eventuele beeldprojector gecontroleerd".into())
            })();
            if let Ok(mut job)=runtime.download.lock(){match result{
                Ok(text)=>{job["status"]=json!(if is_installed(&cfg,entry["id"].as_str().unwrap(),Some(&entry)){"ready"}else{"paused"});job["message"]=json!(text);},
                Err(e)=>{job["status"]=json!("error");job["message"]=json!(e);runtime.queue_paused.store(true,Ordering::Relaxed);}
            }}
        });Ok(())
    }
    pub fn enqueue(&self,key:&str)->Result<()>{
        if !catalog(&self.root)?.iter().any(|m|m["id"]==key&&m["url"].is_string()){return Err("Model cannot be downloaded".into());}
        let job=self.download.lock().map_err(err)?;
        if job["id"]==key&&["downloading","verifying","importing"].iter().any(|s|job["status"]==*s){return Ok(());}
        let mut queue=self.queue.lock().map_err(err)?;if !queue.iter().any(|id|id==key){queue.push(key.into());}Ok(())
    }
    pub fn edit_queue(&self,data:&Value)->Result<()>{
        if data["action"]=="resume"{self.queue_paused.store(false,Ordering::Relaxed);}
        else if data["action"]=="remove"{self.queue.lock().map_err(err)?.retain(|id|Some(id.as_str())!=data["id"].as_str());}
        Ok(())
    }
    /// Searches the Hugging Face API for library models.
    pub fn store_search(&self,data:&Value)->Result<Value>{
        let mut busy=self.queue.lock().map_err(err)?.clone();
        {let job=self.download.lock().map_err(err)?;if let Some(id)=job["id"].as_str(){busy.push(id.to_string());}}
        let loaded={let state=self.state.lock().map_err(err)?;state.active.is_some()};
        crate::store::search(&self.root,data,&busy,loaded)
    }
    fn advance_queue(self:&Arc<Self>){
        if self.queue_paused.load(Ordering::Relaxed){return;}
        let available=self.download.lock().map(|j|!["downloading","verifying","importing"].iter().any(|s|j["status"]==*s)).unwrap_or(false);
        if !available{return;}
        let next=self.queue.lock().ok().and_then(|mut q|if q.is_empty(){None}else{Some(q.remove(0))});
        if let Some(id)=next{if let Err(e)=self.start_download(&id){if let Ok(mut job)=self.download.lock(){*job=json!({"id":id,"status":"error","message":e});}}}
    }
    pub fn pause_download(&self){self.queue_paused.store(true,Ordering::Relaxed);self.cancel.store(true,Ordering::Relaxed);}
    pub fn select_file(&self,slot:&str,file:Option<PathBuf>){if let Ok(mut files)=self.selected_files.lock(){if let Some(file)=file{files.insert(slot.into(),file);}else{files.remove(slot);}}}
    pub fn import_model(self:&Arc<Self>,data:&Value)->Result<String>{
        let name=data["name"].as_str().unwrap_or("").trim().to_string();let category=data["category"].as_str().unwrap_or("chat").to_string();crate::custom_models::validate(&name,&category)?;
        let files=self.selected_files.lock().map_err(err)?.clone();let model=files.get("model").cloned().ok_or("Choose a GGUF file first")?;
        let projector=if category=="vision"{Some(files.get("projector").cloned().ok_or("Also choose the matching GGUF projector")?)}else{None};
        let total=fs::metadata(&model).map_err(err)?.len()+projector.as_ref().map(|p|fs::metadata(p).map(|m|m.len())).transpose().map_err(err)?.unwrap_or(0);
        let id=format!("custom-{}",core::id());let cfg=core::config(&self.root)?;
        {let mut job=self.download.lock().map_err(err)?;if ["downloading","verifying","importing"].iter().any(|v|job["status"]==*v){return Err("Wacht op de huidige download/import of pauzeer die eerst".into());}
        *job=json!({"id":id,"status":"importing","bytes":0,"total":total,"message":"Copy GGUF to USB and calculate SHA-256"});}
        self.cancel.store(false,Ordering::Relaxed);
        let runtime=self.clone();let selected_id=id.clone();let system_role=data["system_role"].as_bool().unwrap_or(true);
        std::thread::spawn(move||{
            let outcome=(||->Result<()>{
                let _lock=core::Lock::acquire("Local\\AIUSB-RustModelDownload")?;
                let directory=core::path(&cfg,"models");
                let mut entry=crate::custom_models::copy_gguf(&model,&directory.join(format!("{selected_id}.gguf")),&runtime.cancel,|bytes|{if let Ok(mut job)=runtime.download.lock(){job["bytes"]=json!(bytes);}})?;
                if let Some(projector)=projector{
                    let offset=entry["bytes"].as_u64().unwrap_or(0);
                    entry["projector"]=crate::custom_models::copy_gguf(&projector,&directory.join(format!("{selected_id}-mmproj.gguf")),&runtime.cancel,|bytes|{if let Ok(mut job)=runtime.download.lock(){job["bytes"]=json!(offset+bytes);}})?;
                }
                entry["id"]=json!(selected_id);entry["name"]=json!(name);entry["category"]=json!(category);entry["description"]=json!("Eigen GGUF-model op USB");entry["system_role"]=json!(system_role);
                crate::custom_models::append(&runtime.root,entry)?;Ok(())
            })();
            if let Ok(mut job)=runtime.download.lock(){match outcome{Ok(())=>{job["status"]=json!("ready");job["message"]=json!("Custom model added to the library");},Err(e)=>{job["status"]=json!("error");job["message"]=json!(e);}}}
        });Ok(id)
    }
}
fn idle_due(active:bool,elapsed:Duration,limit:u64)->bool{active&&elapsed>=Duration::from_secs(limit)}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn vision_waits_for_verified_projector(){
        let dir=core::root().unwrap().join("tools/temp").join(format!("projector-{}",core::id()));fs::create_dir_all(&dir).unwrap();
        let model=dir.join("model.gguf");let projector=dir.join("projector.gguf");let metadata=json!({"bytes":4,"sha256":"a".repeat(64),"projector":{"file":"projector.gguf","bytes":4,"sha256":"b".repeat(64)}});
        let cfg=json!({"paths":{"models":dir},"services":{"vision":{"args":["--model",model]}}});
        fs::write(&model,b"GGUF").unwrap();core::atomic(&model.with_extension("gguf.verified.json"),&metadata).unwrap();
        assert!(downloaded_file_valid(&model,&metadata));assert!(!is_installed(&cfg,"vision",Some(&metadata)));
        fs::write(&projector,b"GGUF").unwrap();assert!(!is_installed(&cfg,"vision",Some(&metadata)));
        core::atomic(&projector.with_extension("gguf.verified.json"),&metadata["projector"]).unwrap();assert!(is_installed(&cfg,"vision",Some(&metadata)));
        fs::write(&projector,b"bad").unwrap();assert!(!is_installed(&cfg,"vision",Some(&metadata)));fs::remove_dir_all(dir).unwrap();
    }
    #[test]fn idle_policy(){assert!(!idle_due(false,Duration::from_secs(1801),1800));assert!(!idle_due(true,Duration::from_secs(90),1800));assert!(!idle_due(true,Duration::from_secs(1799),1800));assert!(idle_due(true,Duration::from_secs(1800),1800));}
    #[test]fn custom_profile_preserves_lower_limits_and_defaults(){
        let entry=json!({"system_prompt":"Je heet Orvyn 1.","generation":{"max_tokens":768,"temperature":0.3,"top_p":0.9}});
        let mut request=json!({"messages":[],"max_tokens":4096});
        assert_eq!(prepare_profile(&mut request,Some(&entry)),"Je heet Orvyn 1.");
        assert_eq!(request["max_tokens"],768);assert_eq!(request["temperature"],0.3);
        request["max_tokens"]=json!(32);request["temperature"]=json!(0.8);
        prepare_profile(&mut request,Some(&entry));
        assert_eq!(request["max_tokens"],32);assert_eq!(request["temperature"],0.8);
        let mut ordinary=json!({"max_tokens":1500});
        assert_eq!(prepare_profile(&mut ordinary,None),CHAT_INSTRUCTIONS);
        assert_eq!(ordinary,json!({"max_tokens":1500}));
    }
    #[test]fn catalog_is_complete(){
        let project=core::root().unwrap();
        let root=project.join("tools/temp").join(format!("catalog-{}",core::id()));
        fs::create_dir_all(root.join("config")).unwrap();
        for name in ["settings","custom-models","model-catalog"]{
            fs::copy(project.join(format!("config/{name}.example.json")),root.join(format!("config/{name}.json"))).unwrap();
        }
        let entries=catalog(&root).unwrap();
        assert_eq!(entries.len(),1);
        assert_eq!(entries[0]["id"],"orvyn-1");
        for m in entries{
            let sha=m["sha256"].as_str().unwrap();
            assert_eq!(sha.len(),64);
            assert!(sha.bytes().all(|b|b.is_ascii_hexdigit()));
            assert!(m["url"].as_str().unwrap().starts_with("https://"));
        }
        let cfg=core::config(&root).unwrap();
        assert_eq!(cfg["services"].as_object().unwrap().len(),1);
        assert!(cfg["services"]["orvyn-1"].is_object());
        fs::remove_dir_all(root).unwrap();
    }
}
