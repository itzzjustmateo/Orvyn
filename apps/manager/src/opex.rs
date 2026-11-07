use crate::core::{self,err,Result};
use serde_json::{json,Value};
use std::{fs,path::{Path,PathBuf},io::Read,process::Stdio,time::{Instant,Duration}};
use std::sync::{Arc,Mutex,OnceLock,atomic::{AtomicBool,Ordering}};
use std::collections::BTreeMap;

struct Job { cancel:Arc<AtomicBool>, state:Arc<Mutex<Value>> }
static JOBS:OnceLock<Mutex<BTreeMap<String,Job>>>=OnceLock::new();
fn jobs()->&'static Mutex<BTreeMap<String,Job>>{JOBS.get_or_init(||Mutex::new(BTreeMap::new()))}

pub fn tree(root:&Path,input:&Value)->Result<Value>{
    fn walk(base:&Path,dir:&Path,depth:usize,files:&mut Vec<Value>,truncated:&mut bool)->Result<()>{
        if depth>6||files.len()>=1000{*truncated=true;return Ok(());}
        let mut entries=fs::read_dir(dir).map_err(err)?.collect::<std::result::Result<Vec<_>,_>>().map_err(err)?;
        entries.sort_by_key(|e|e.file_name());
        for entry in entries{
            if files.len()>=1000{*truncated=true;break;}
            let name=entry.file_name().to_string_lossy().to_string();
            if [".git","node_modules","target","build-usb",".next",".venv","webview-profile"].contains(&name.as_str()){continue;}
            let kind=entry.file_type().map_err(err)?;
            if kind.is_symlink(){continue;}
            let path=fs::canonicalize(entry.path()).map_err(err)?;
            if !path.starts_with(base){continue;}
            let relative=path.strip_prefix(base).map_err(err)?.to_string_lossy().replace('\\',"/");
            files.push(json!({"path":relative,"directory":kind.is_dir()}));
            if kind.is_dir(){walk(base,&path,depth+1,files,truncated)?;}
        }
        Ok(())
    }
    let dir=folder(root,input)?;let mut files=vec![];let mut truncated=false;
    walk(&dir,&dir,0,&mut files,&mut truncated)?;
    Ok(json!({"folder":dir,"files":files,"truncated":truncated}))
}

pub fn start_job(root:&Path,input:&Value)->Result<Value>{
    if input["action"]!="run"||input["approved"]!=true{return Err("Confirm the terminal command first.".into());}
    let mut p=input.clone();let obj=p.as_object_mut().ok_or("Invalid request")?;obj.remove("folder");obj.remove("approved");proposal(&p)?;
    let dir=folder(root,input)?;let command=input["command"].as_str().ok_or("Command is missing")?.to_owned();
    let mut registry=jobs().lock().map_err(err)?;
    if registry.values().any(|j|j.state.lock().map(|s|s["status"]=="running").unwrap_or(true)){return Err("An Opex command is already running. Stop it or wait for it to finish.".into());}
    if registry.len()>=20{registry.clear();}
    let id=core::id();let cancel=Arc::new(AtomicBool::new(false));let state=Arc::new(Mutex::new(json!({"status":"running","output":""})));
    registry.insert(id.clone(),Job{cancel:cancel.clone(),state:state.clone()});
    let root=root.to_owned();
    std::thread::spawn(move||{
        let result=run_command(&root,&dir,&command,Some(&cancel),Some(&state));
        if let Ok(mut value)=state.lock(){*value=match result{Ok(mut v)=>{v["status"]=json!("finished");v},Err(e)=>json!({"status":"failed","error":e})};}
    });
    Ok(json!({"id":id}))
}
pub fn job(input:&Value,stop:bool)->Result<Value>{
    let id=input["id"].as_str().ok_or("Task ID is missing")?;
    let registry=jobs().lock().map_err(err)?;let job=registry.get(id).ok_or("Terminal task not found")?;
    if stop{job.cancel.store(true,Ordering::SeqCst);}
    let value=job.state.lock().map_err(err)?.clone();Ok(value)
}
fn run_command(root:&Path,dir:&Path,command:&str,cancel:Option<&AtomicBool>,state:Option<&Mutex<Value>>)->Result<Value>{
    let logs=root.join("logs").join("opex");fs::create_dir_all(&logs).map_err(err)?;
    let log=logs.join(format!("{}.log",core::id()));let file=fs::File::create(&log).map_err(err)?;
    let temp=root.join("tools/temp");fs::create_dir_all(&temp).map_err(err)?;
    if cancel.is_some_and(|v|v.load(Ordering::SeqCst)){return Ok(json!({"cancelled":true,"output":"","exit_code":null}));}
    let mut child=core::hidden("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",command]).current_dir(dir).env("TEMP",&temp).env("TMP",&temp).stdin(Stdio::null()).stdout(file.try_clone().map_err(err)?).stderr(file).spawn().map_err(err)?;
    let start=Instant::now();let mut last=Instant::now();let mut timed_out=false;let mut cancelled=false;
    let code=loop{
        if let Some(status)=child.try_wait().map_err(err)?{break status.code();}
        cancelled=cancel.is_some_and(|v|v.load(Ordering::SeqCst));timed_out=start.elapsed()>Duration::from_secs(60);
        if cancelled||timed_out{let _=core::hidden("taskkill.exe").args(["/PID",&child.id().to_string(),"/T","/F"]).output();let _=child.kill();let _=child.wait();break None;}
        if last.elapsed()>Duration::from_millis(500){
            if let Some(state)=state{if let Ok(mut value)=state.lock(){value["output"]=json!(log_output(&log).unwrap_or_default());}}
            last=Instant::now();
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    Ok(json!({"exit_code":code,"timed_out":timed_out,"cancelled":cancelled,"output":log_output(&log)?,"log":log}))
}
fn log_output(path:&Path)->Result<String>{let mut bytes=vec![];fs::File::open(path).map_err(err)?.take(32000).read_to_end(&mut bytes).map_err(err)?;Ok(String::from_utf8_lossy(&bytes).to_string())}
fn folder(root:&Path,input:&Value)->Result<PathBuf>{
    let path=input["folder"].as_str().ok_or("Project folder is missing")?;
    let path=fs::canonicalize(path).map_err(err)?;
    let usb=fs::canonicalize(root).map_err(err)?;
    if !path.starts_with(&usb)||!path.is_dir(){return Err("Choose a project folder on this USB.".into());}
    Ok(path)
}
fn target(folder:&Path,name:&str)->Result<PathBuf>{
    relative_path(name)?;
    let relative=Path::new(name);
    if relative.as_os_str().is_empty()||relative.components().any(|c|!matches!(c,std::path::Component::Normal(_)))||name.contains(':'){return Err("Use a relative file path without ..".into());}
    let p=folder.join(relative);
    let resolved=if p.exists(){fs::canonicalize(&p).map_err(err)?}else{fs::canonicalize(p.parent().ok_or("Invalid path")?).map_err(err)?.join(p.file_name().ok_or("Invalid file")?)};
    if !resolved.starts_with(folder){return Err("Pad verlaat de projectmap.".into());}
    Ok(resolved)
}
fn relative_path(name:&str)->Result<()> {
    if name.trim().is_empty()||name.len()>500||name.chars().any(|c|c.is_control())||name.contains(':')||name.starts_with(['/', '\\']) {
        return Err("Use a relative file path inside the project folder.".into());
    }
    for part in name.split(['/', '\\']) {
        let stem=part.split('.').next().unwrap_or("").to_ascii_uppercase();
        let reserved=["CON","PRN","AUX","NUL"].contains(&stem.as_str())||
            (stem.len()==4&&(stem.starts_with("COM")||stem.starts_with("LPT"))&&matches!(stem.as_bytes()[3],b'1'..=b'9'));
        if part.is_empty()||part=="."||part==".."||part.ends_with([' ','.'])||part.contains(['*','?','"','<','>','|'])||reserved {
            return Err("Invalid file path: do not use .., reserved device names, or wildcards.".into());
        }
    }
    Ok(())
}
pub fn proposal(value:&Value)->Result<Value> {
    let obj=value.as_object().ok_or("Opex must return one JSON object.")?;
    let allowed:&[&str]=if obj.contains_key("answer"){&["answer"]}else{match value["action"].as_str(){
        Some("read")=>&["action","path"],Some("write")=>&["action","path","text"],Some("run")=>&["action","command"],
        _=>return Err("Invalid Opex action. Expected read, write, run, or an answer.".into())
    }};
    if obj.len()!=allowed.len()||obj.keys().any(|k|!allowed.contains(&k.as_str())) {return Err("Opex proposal has missing or unexpected fields; approval and project folder must not come from the model.".into());}
    for key in allowed {if !value[*key].is_string(){return Err(format!("Field {key} must be text."));}}
    if let Some(path)=value["path"].as_str(){relative_path(path)?;}
    if value["text"].as_str().is_some_and(|s|s.len()>100_000){return Err("Proposal groter dan 100 KB.".into());}
    for key in ["answer","command"] {if let Some(s)=value[key].as_str(){if s.trim().is_empty()||s.len()>8000||s.contains('\0'){return Err(format!("Invalid or excessively long field: {key}."));}}}
    Ok(value.clone())
}
pub fn validate(root:&Path,input:&Value)->Result<Value>{
    let value=proposal(&input["proposal"])?;
    let dir=folder(root,input)?;
    if let Some(path)=value["path"].as_str(){
        let p=target(&dir,path)?;
        if p.exists()&&(!p.is_file()||fs::metadata(&p).map_err(err)?.len()>100_000){return Err("Choose a text file of up to 100 KB.".into());}
    }
    Ok(json!({"valid":true,"proposal":value,"requires_approval":value["action"]=="write"||value["action"]=="run"}))
}
pub fn action(root:&Path,input:&Value)->Result<Value>{
    let obj=input.as_object().ok_or("Invalid Opex request")?;
    let mut proposed=input.clone();
    if let Some(p)=proposed.as_object_mut(){p.remove("folder");p.remove("approved");p.remove("base");}
    if input["action"]!="list"{proposal(&proposed)?;}else if obj.keys().any(|k|!["folder","action"].contains(&k.as_str())){return Err("Onverwachte velden bij list.".into());}
    if obj.contains_key("approved")&&!input["approved"].is_boolean(){return Err("Approval must be true or false.".into());}
    if obj.contains_key("base")&&input["action"]!="write"{return Err("A base version is only valid for write actions.".into());}
    let dir=folder(root,input)?;
    match input["action"].as_str().unwrap_or(""){
        "list"=>{let mut files=vec![];for entry in fs::read_dir(&dir).map_err(err)?.take(300){let entry=entry.map_err(err)?;files.push(json!({"name":entry.file_name().to_string_lossy(),"directory":entry.file_type().map_err(err)?.is_dir()}));}Ok(json!({"folder":dir,"files":files}))},
        "read"=>{let p=target(&dir,input["path"].as_str().ok_or("Path is missing")?)?;if !p.exists(){return Ok(json!({"path":p,"text":"","exists":false}));}if fs::metadata(&p).map_err(err)?.len()>100_000{return Err("File exceeds 100 KB.".into());}Ok(json!({"path":p,"text":fs::read_to_string(p).map_err(err)?}))},
        "write"=>{
            if input["approved"]!=true{return Err("Bevestig eerst de bestandswijziging.".into());}
            let p=target(&dir,input["path"].as_str().ok_or("Path is missing")?)?;
            let text=input["text"].as_str().ok_or("Content is missing")?;
            if text.len()>100_000{return Err("Proposal te groot.".into());}
            let current=if p.exists(){fs::read_to_string(&p).map_err(err)?}else{String::new()};
            if input["base"].as_str()!=Some(current.as_str()){return Err("The file has changed. Read it again before saving.".into());}
            let backup=root.join("backups").join(format!("opex-{}",core::id()));
            fs::create_dir_all(&backup).map_err(err)?;
            fs::write(backup.join("original.txt"),&current).map_err(err)?;
            core::atomic(&backup.join("manifest.json"),&json!({"path":p,"existed":p.exists()}))?;
            fs::write(&p,text).map_err(err)?;
            Ok(json!({"saved":p,"backup":backup}))
        },
        "run"=>{
            if input["approved"]!=true{return Err("Confirm the terminal command first.".into());}
            let command=input["command"].as_str().filter(|s|!s.trim().is_empty()&&s.len()<8000).ok_or("Invalid command")?;
            run_command(root,&dir,command,None,None)
        },
        _=>Err("Onbekende Opex-actie".into())
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn write_backup_and_stale_base(){
        let root=core::root().unwrap();
        let dir=root.join("tools/temp").join(format!("opex-test-{}",core::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("sample.txt"),"before").unwrap();
        let mut data=json!({"folder":dir,"action":"write","path":"sample.txt","text":"after","base":"before"});
        assert!(action(&root,&data).is_err());
        data["approved"]=json!(true);
        let result=action(&root,&data).unwrap();
        assert_eq!(fs::read_to_string(dir.join("sample.txt")).unwrap(),"after");
        assert_eq!(fs::read_to_string(Path::new(result["backup"].as_str().unwrap()).join("original.txt")).unwrap(),"before");
        assert!(action(&root,&data).unwrap_err().contains("changed"));
        data=json!({"folder":dir,"action":"read","path":"new.txt"});
        assert_eq!(action(&root,&data).unwrap()["exists"],false);
        fs::remove_file(dir.join("sample.txt")).unwrap();fs::remove_dir(dir).unwrap();
    }
    #[test] fn traversal_rejected(){assert!(target(Path::new("E:/"),"../secret").is_err());assert!(target(Path::new("E:/"),"C:/test").is_err());assert!(target(Path::new("E:/"),"test:stream").is_err());}
    #[test] fn approval_required(){let root=core::root().unwrap();let data=json!({"folder":root,"action":"run","command":"exit 0"});assert!(action(&root,&data).unwrap_err().contains("Confirm"));}
    #[test] fn malformed_proposals_are_blocked(){
        for v in [json!(null),json!([]),json!({"action":"delete","path":"x"}),json!({"action":"run","command":" "}),json!({"action":"read","path":7}),json!({"action":"run","command":"echo x","approved":true}),json!({"answer":"ok","action":"run","command":"echo x"}),json!({"action":"write","path":"x"})]{assert!(proposal(&v).is_err(),"{v}");}
        for p in ["../x","a/../../x","C:\\x","\\\\host\\share","NUL.txt","x:stream","a/CON","a/","x.","x\n"]{assert!(proposal(&json!({"action":"read","path":p})).is_err(),"{p}");}
        assert!(proposal(&json!({"action":"read","path":"src/main.rs"})).is_ok());
        assert!(proposal(&json!({"action":"write","path":"empty.txt","text":""})).is_ok());
    }
}
