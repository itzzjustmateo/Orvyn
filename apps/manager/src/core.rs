use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::os::windows::process::CommandExt;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub type Result<T> = std::result::Result<T, String>;
pub fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn id() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}
pub fn hidden(command: &str) -> Command {
    let mut c = Command::new(command);
    c.creation_flags(0x08000000);
    c
}
pub fn root() -> Result<PathBuf> {
    let exe = std::env::current_exe().map_err(err)?;
    exe.ancestors()
        .find(|p| p.join("config/settings.json").is_file())
        .map(Path::to_path_buf)
        .ok_or("config/settings.json is missing".into())
}
pub fn read_json(path: &Path) -> Result<Value> {
    serde_json::from_slice(&fs::read(path).map_err(err)?).map_err(err)
}
pub fn config(root: &Path) -> Result<Value> {
    let mut c = read_json(&root.join("config/settings.json"))?;
    crate::models::extend_config(root,&mut c)?;
    validate(&c)?;
    Ok(resolve_config(root, c))
}
fn resolve_config(root: &Path, mut c: Value) -> Value {
    let resolve = |s: &str| -> String {
        let p = Path::new(s);
        if p.is_absolute() {
            s.into()
        } else {
            root.join(p).to_string_lossy().into_owned()
        }
    };
    if let Some(paths) = c["paths"].as_object_mut() {
        for value in paths.values_mut() {
            if let Some(s) = value.as_str() {
                *value = json!(resolve(s));
            }
        }
    }
    if let Some(services) = c["services"].as_object_mut() {
        for s in services.values_mut() {
            if let Some(cmd) = s["command"].as_str() {
                if cmd.contains('/') || cmd.contains('\\') {
                    s["command"] = json!(resolve(cmd));
                }
            }
            if let Some(cwd) = s["cwd"].as_str() {
                s["cwd"] = json!(resolve(cwd));
            }
            if let Some(args) = s["args"].as_array_mut() {
                for arg in args {
                    if let Some(a) = arg.as_str() {
                        if a.ends_with(".gguf") {
                            *arg = json!(resolve(a));
                        }
                    }
                }
            }
        }
    }
    c["_root"] = json!(root);
    c
}
fn portable_config(cfg: &Value) -> Value {
    fn visit(v: &mut Value, root: &Path) {
        match v {
            Value::String(s) => {
                if let Ok(p) = Path::new(s).strip_prefix(root) {
                    if !p.as_os_str().is_empty() {
                        *s = p.to_string_lossy().replace('\\', "/");
                    }
                }
            }
            Value::Array(a) => {
                for x in a {
                    visit(x, root)
                }
            }
            Value::Object(o) => {
                for x in o.values_mut() {
                    visit(x, root)
                }
            }
            _ => {}
        }
    }
    let mut value = cfg.clone();
    if let Some(root) = cfg["_root"].as_str() {
        value.as_object_mut().unwrap().remove("_root");
        visit(&mut value, Path::new(root));
    }
    value
}
pub fn path(cfg: &Value, key: &str) -> PathBuf {
    PathBuf::from(cfg["paths"][key].as_str().unwrap_or(""))
}
pub fn validate(cfg: &Value) -> Result<()> {
    let mut ports = std::collections::HashSet::new();
    let services = cfg["services"].as_object().ok_or("services is missing")?;
    for (key, s) in services {
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err("Invalid service ID".into());
        }
        let port = s["port"].as_u64().ok_or("Port must be a number")?;
        if port == 0 || port > 65535 || !ports.insert(port) {
            return Err("Ports must be unique (1-65535)".into());
        }
        if s["command"].as_str().is_none()
            || !s["args"]
                .as_array()
                .is_some_and(|a| a.iter().all(Value::is_string))
        {
            return Err("Invalid service command".into());
        }
    }
    for key in ["data", "logs", "models", "backups"] {
        if path(cfg, key).as_os_str().is_empty() {
            return Err(format!("Path {key} is missing"));
        }
    }
    Ok(())
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateJobObjectW(attr:*const u8,name:*const u16)->isize;
    fn SetInformationJobObject(job:isize,kind:u32,info:*const u8,size:u32)->i32;
    fn AssignProcessToJobObject(job:isize,process:isize)->i32;
    fn MoveFileExW(a: *const u16, b: *const u16, flags: u32) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
    fn GetProcessTimes(h: isize, a: *mut u64, b: *mut u64, c: *mut u64, d: *mut u64) -> i32;
    fn CloseHandle(h: isize) -> i32;
    fn CreateMutexW(attr: *const u8, owner: i32, name: *const u16) -> isize;
    fn WaitForSingleObject(h: isize, timeout: u32) -> u32;
    fn ReleaseMutex(h: isize) -> i32;
}
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub struct Lock(isize);
#[repr(C)]
struct BasicJobLimits { process_time:i64,job_time:i64,flags:u32,min_working:usize,max_working:usize,active:u32,affinity:usize,priority:u32,scheduling:u32 }
#[repr(C)]
struct JobLimits { basic:BasicJobLimits,io:[u64;6],process_memory:usize,job_memory:usize,peak_process:usize,peak_job:usize }
pub struct ProcessJob(isize);
impl ProcessJob {
    pub fn new()->Result<Self>{unsafe{
        let job=CreateJobObjectW(std::ptr::null(),std::ptr::null());
        if job==0{return Err(err(std::io::Error::last_os_error()));}
        let mut limits:JobLimits=std::mem::zeroed();limits.basic.flags=0x2000;
        if SetInformationJobObject(job,9,(&limits as *const JobLimits).cast(),std::mem::size_of::<JobLimits>() as u32)==0 || AssignProcessToJobObject(job,-1)==0 {
            CloseHandle(job);return Err(err(std::io::Error::last_os_error()));
        }
        Ok(Self(job))
    }}
}
impl Drop for ProcessJob {fn drop(&mut self){unsafe{CloseHandle(self.0);}}}
impl Lock {
    pub fn acquire(name: &str) -> Result<Self> {
        unsafe {
            let h = CreateMutexW(std::ptr::null(), 0, wide(name).as_ptr());
            if h == 0 {
                return Err(err(std::io::Error::last_os_error()));
            }
            let status = WaitForSingleObject(h, 5000);
            if status != 0 && status != 0x80 {
                CloseHandle(h);
                return Err("Another action is still running".into());
            }
            Ok(Self(h))
        }
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}
pub fn atomic(path: &Path, value: &Value) -> Result<()> {
    atomic_write(path, value, true)
}
// Only for disposable session state. Conversations/configuration must use atomic().
pub fn atomic_transient(path: &Path, value: &Value) -> Result<()> {
    atomic_write(path, value, false)
}
fn atomic_write(path: &Path, value: &Value, durable: bool) -> Result<()> {
    fs::create_dir_all(path.parent().ok_or("No parent folder")?).map_err(err)?;
    let temp = path.with_extension(format!("{}.tmp", id()));
    let result = (|| {
        let mut f = File::create(&temp).map_err(err)?;
        f.write_all(&serde_json::to_vec_pretty(value).map_err(err)?)
            .map_err(err)?;
        if durable { f.sync_all().map_err(err)?; }
        drop(f);
        if unsafe {
            MoveFileExW(
                wide(&temp.to_string_lossy()).as_ptr(),
                wide(&path.to_string_lossy()).as_ptr(),
                if durable { 9 } else { 1 },
            )
        } == 0
        {
            return Err(err(std::io::Error::last_os_error()));
        }
        Ok(())
    })();
    let _ = fs::remove_file(temp);
    result
}
fn identity(pid: u32) -> Option<u64> {
    unsafe {
        let h = OpenProcess(0x1000, 0, pid);
        if h == 0 {
            return None;
        }
        let (mut created, mut exited, mut kernel, mut user) = (0, 0, 0, 0);
        let ok = GetProcessTimes(h, &mut created, &mut exited, &mut kernel, &mut user);
        CloseHandle(h);
        if ok != 0 && exited == 0 {
            Some(created)
        } else {
            None
        }
    }
}
pub fn online(cfg: &Value, key: &str) -> bool {
    let p = cfg["services"][key]["port"].as_u64().unwrap_or(0) as u16;
    TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], p)),
        Duration::from_millis(250),
    )
    .is_ok()
}
pub fn service_owned(cfg:&Value,key:&str)->bool {
    read_json(&path(cfg,"data").join("processes").join(format!("{key}.rust.json"))).ok().is_some_and(|v| {
        v["pid"].as_u64().is_some_and(|pid|identity(pid as u32).is_some_and(|created|Some(created)==v["created"].as_u64()))
    })
}
pub fn service(root: &Path, key: &str, start: bool) -> Result<String> {
    let _lock = Lock::acquire("Local\\AIUSB-RustServices")?;
    let cfg = config(root)?;
    let svc = &cfg["services"][key];
    if svc.is_null() {
        return Err("Onbekende service".into());
    }
    let pidfile = path(&cfg, "data")
        .join("processes")
        .join(format!("{key}.rust.json"));
    let owned = read_json(&pidfile).ok().and_then(|v| {
        let pid = v["pid"].as_u64()? as u32;
        if identity(pid) == v["created"].as_u64() {
            Some(pid)
        } else {
            None
        }
    });
    if !start {
        if let Some(pid) = owned {
            let output = hidden("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .output()
                .map_err(err)?;
            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).into());
            }
            let _ = fs::remove_file(pidfile);
            return Ok(format!("{key} stopped"));
        }
        return if online(&cfg, key) {
            Err(format!(
                "{key} is running outside this Rust manager; close the original process first"
            ))
        } else {
            Ok(format!("{key} has stopped"))
        };
    }
    if online(&cfg,key) && owned.is_none() {
        return Err(format!("Port for {key} is used by another process"));
    }
    if owned.is_some() {
        return Ok(format!("{key} is already running or loading"));
    }
    let mut args: Vec<String> = svc["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().into())
        .collect();
    if let Some(i) = args.iter().position(|s| s == "--port") {
        if i + 1 < args.len() {
            args[i + 1] = svc["port"].to_string();
        }
    } else if key == "gui" {
        args.extend([
            "--port".into(),
            svc["port"].to_string(),
            "--hostname".into(),
            "127.0.0.1".into(),
        ]);
    }
    fs::create_dir_all(path(&cfg, "logs")).map_err(err)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path(&cfg, "logs").join(format!("{key}.log")))
        .map_err(err)?;
    let mut child = hidden(svc["command"].as_str().unwrap())
        .args(args)
        .current_dir(
            svc["cwd"]
                .as_str()
                .map(PathBuf::from)
                .unwrap_or(root.into()),
        )
        .env("AI_USB_CONFIG", root.join("config/settings.json"))
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(err)?)
        .stderr(log)
        .spawn()
        .map_err(err)?;
    std::thread::sleep(Duration::from_millis(200));
    if let Some(code) = child.try_wait().map_err(err)? {
        return Err(format!("{key} stopped ({code}); check the log"));
    }
    let pid = child.id();
    atomic(
        &pidfile,
        &json!({"pid":pid,"created": identity(pid).ok_or("Process stopped during startup")?}),
    )?;
    Ok(format!("{key} started; the model may still be loading"))
}
pub fn load_chats(cfg: &Value) -> Result<Value> {
    let file = path(cfg, "data").join("chats.json");
    if file.exists() {
        read_json(&file)
    } else {
        Ok(json!({"chats":[],"revision":null}))
    }
}
pub fn save_chats(cfg: &Value, state: &mut Value) -> Result<()> {
    let _lock = Lock::acquire("Local\\AIUSB-RustChatStorage")?;
    let disk = load_chats(cfg)?;
    if disk["revision"] != state["revision"] {
        return Err(
            "Conversations changed elsewhere. Click Reload before continuing.".into(),
        );
    }
    state["revision"] = Value::String(id());
    atomic(&path(cfg, "data").join("chats.json"), state)
}
pub fn chat(cfg: &Value, key: &str, messages: Value) -> Result<String> {
    let response = chat_response(
        cfg,
        key,
        json!({"model":key,"messages":messages,"max_tokens":4096,"stream":false}),
    )?;
    let text = response["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim();
    if text.is_empty() {
        return Err("The model did not produce a final answer within the token budget.".into());
    }
    if text.contains("@@@@@@") || text.contains('\u{001f}') || text.contains("<|im_start|>") {
        return Err("Invalid model output; check the settings.".into());
    }
    Ok(text.into())
}
pub fn chat_response(cfg: &Value, key: &str, mut body: Value) -> Result<Value> {
    body["stream"] = json!(false);
    let port = cfg["services"][key]["port"]
        .as_u64()
        .ok_or("Onbekend model")?;
    let mut child = hidden("curl.exe")
        .args([
            "--silent",
            "--show-error",
            "--fail-with-body",
            "--max-time",
            "300",
            "--noproxy",
            "*",
            "-H",
            "Content-Type: application/json",
            "--data-binary",
            "@-",
            &format!("http://127.0.0.1:{port}/v1/chat/completions"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(err)?;
    child
        .stdin
        .take()
        .ok_or("No input pipe")?
        .write_all(body.to_string().as_bytes())
        .map_err(err)?;
    let output = child.wait_with_output().map_err(err)?;
    if !output.status.success() {
        return Err(format!(
            "Model request failed: {} {}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        ));
    }
    let response: Value = serde_json::from_slice(&output.stdout).map_err(err)?;
    Ok(response)
}
pub fn stream_response(cfg:&Value,key:&str,mut body:Value,mut emit:impl FnMut(String)->Result<()>)->Result<Value>{
    use std::io::{BufRead,BufReader};
    body["stream"]=json!(true);body["stream_options"]=json!({"include_usage":true});
    let port=cfg["services"][key]["port"].as_u64().ok_or("Onbekend model")?;
    let mut child=hidden("curl.exe").args(["-sS","-N","--fail-with-body","--max-time","300","--noproxy","*","-H","Content-Type: application/json","--data-binary","@-",&format!("http://127.0.0.1:{port}/v1/chat/completions")]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(err)?;
    child.stdin.take().ok_or("Input is missing")?.write_all(body.to_string().as_bytes()).map_err(err)?;
    let mut content=String::new();let mut usage=Value::Null;let mut complete=false;
    let result=(||->Result<()>{
        for line in BufReader::new(child.stdout.take().ok_or("Output is missing")?).lines(){
            let line=line.map_err(err)?;let Some(data)=line.strip_prefix("data:")else{continue};let data=data.trim();
            if data=="[DONE]"{complete=true;continue;}
            let chunk:Value=serde_json::from_str(data).map_err(err)?;
            if !chunk["error"].is_null(){return Err(chunk["error"].to_string());}
            if let Some(text)=chunk["choices"][0]["delta"]["content"].as_str(){content.push_str(text);}
            if !chunk["usage"].is_null(){usage=chunk["usage"].clone();}
            emit(format!("data: {data}\n\n"))?;
        }Ok(())
    })();
    if result.is_err(){let _=child.kill();}let status=child.wait().map_err(err)?;result?;
    if !status.success()||!complete{return Err("Model stream interrupted or unsupported by the runtime".into());}
    Ok(json!({"choices":[{"message":{"content":content}}],"usage":usage}))
}
pub fn backup(cfg: &Value) -> Result<PathBuf> {
    let dir = path(cfg, "backups");
    fs::create_dir_all(&dir).map_err(err)?;
    let file = dir.join(format!("ai-usb-{}.zip", id()));
    let mut zip = zip::ZipWriter::new(File::create(&file).map_err(err)?);
    let opts =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, value) in [
        ("manifest.json", json!({"version":1})),
        ("settings.json", portable_config(cfg)),
        ("chats.json", load_chats(cfg)?),
    ] {
        zip.start_file(name, opts).map_err(err)?;
        zip.write_all(value.to_string().as_bytes()).map_err(err)?;
    }
    if let Some(root)=cfg["_root"].as_str(){
        for name in ["custom-models.json","store-models.json","api-providers.json"]{let file=Path::new(root).join("config").join(name);if file.exists(){zip.start_file(name,opts).map_err(err)?;zip.write_all(&fs::read(file).map_err(err)?).map_err(err)?;}}
    }
    zip.finish().map_err(err)?;
    Ok(file)
}
pub fn restore(cfg: &Value, root: &Path, file: &Path) -> Result<String> {
    fn catalog_valid(data: &Value) -> bool {
        data["models"].as_array().is_some_and(|entries| {
            entries.iter().all(|m| {
                m["id"].is_string()
                    && m["file"].as_str().is_some_and(|f| {
                        f.ends_with(".gguf")
                            && f.bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                    })
            })
        })
    }
    let mut zip = zip::ZipArchive::new(File::open(file).map_err(err)?).map_err(err)?;
    let has_custom=zip.file_names().any(|name|name=="custom-models.json");
    let has_store=zip.file_names().any(|name|name=="store-models.json");
    let has_providers=zip.file_names().any(|name|name=="api-providers.json");
    let mut read = |name: &str| -> Result<Value> {
        let mut entry = zip.by_name(name).map_err(err)?;
        if entry.size() > 50_000_000 {
            return Err("Back-up te groot".into());
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(err)?;
        serde_json::from_slice(&bytes).map_err(err)
    };
    if read("manifest.json")?["version"] != 1 {
        return Err("Onbekende back-upversie".into());
    }
    let restored = read("settings.json")?;
    validate(&restored)?;
    let mut state = read("chats.json")?;
    let custom=if has_custom{Some(read("custom-models.json")?)}else{None};
    let store=if has_store{Some(read("store-models.json")?)}else{None};
    if let Some(ref custom)=custom{
        if !catalog_valid(custom){return Err("Invalid custom model catalog in backup".into());}
    }
    if let Some(ref store)=store{
        if !catalog_valid(store){return Err("Invalid store model catalog in backup".into());}
    }
    if !state["chats"].is_array() {
        return Err("Invalid conversations".into());
    }
    let providers=if has_providers{let data=read("api-providers.json")?;crate::providers::validate_config(&data)?;Some(data)}else{None};
    let previous = backup(cfg)?;
    state["revision"] = Value::String(id());
    atomic(
        &path(&resolve_config(root, restored.clone()), "data").join("chats.json"),
        &state,
    )?;
    atomic(&root.join("config/settings.json"), &restored)?;
    if let Some(custom)=custom{atomic(&root.join("config/custom-models.json"),&custom)?;}
    if let Some(store)=store{atomic(&root.join("config/store-models.json"),&store)?;}
    if let Some(providers)=providers{atomic(&root.join("config/api-providers.json"),&providers)?;}
    Ok(format!(
        "Restored. Previous version: {}. Reload chats and restart affected services.",
        previous.display()
    ))
}
pub fn download(
    cfg: &Value,
    url: &str,
    name: &str,
    expected: &str,
    cancel: Arc<AtomicBool>,
    report: impl Fn(String),
) -> Result<String> {
    let _lock = Lock::acquire("Local\\AIUSB-RustModelDownload")?;
    if !url.starts_with("https://")
        || expected.len() != 64
        || !expected.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("An HTTPS URL and a 64-digit hexadecimal SHA-256 are required".into());
    }
    if !name.ends_with(".gguf")
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err("Use a .gguf filename without folders".into());
    }
    let dir = path(cfg, "models");
    fs::create_dir_all(&dir).map_err(err)?;
    let target = dir.join(name);
    if target.exists() {
        return Err("Doelbestand bestaat al".into());
    }
    let part = dir.join(format!("{name}.part"));
    let meta = dir.join(format!("{name}.download.json"));
    let source = json!({"url":url,"sha256":expected.to_lowercase()});
    if part.exists() && read_json(&meta)? != source {
        return Err("The partial download belongs to a different URL/hash".into());
    }
    atomic(&meta, &source)?;
    let mut retries=0;
    'transfer: loop {
    if cancel.load(Ordering::Relaxed) {return Ok("Download paused".into());}
    let mut child = hidden("curl.exe")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--location",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--connect-timeout",
            "30",
            "--speed-time",
            "60",
            "--speed-limit",
            "1024",
            "--continue-at",
            "-",
            "--write-out",
            "%{http_code}",
            "--output",
        ])
        .arg(&part)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(err)?;
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Ok("Paused; use the same URL/name/hash to resume the download".into());
        }
        if let Some(status) = child.try_wait().map_err(err)? {
            let mut http = String::new();
            if let Some(mut stdout) = child.stdout.take() {
                stdout.read_to_string(&mut http).map_err(err)?;
            }
            // A fully downloaded .part may receive 416 on resume. It still must
            // pass the complete SHA-256 check before becoming a model.
            if !status.success() && http.trim() != "416" {
                if retries<3 && (matches!(status.code(),Some(5|6|7|18|28|35|52|55|56|92)) || matches!(http.trim(),"429"|"500"|"502"|"503"|"504")) {
                    retries+=1;
                    report(format!("Verbinding onderbroken. Automatisch hervatten ({retries}/3)…"));
                    for _ in 0..(20*retries) {if cancel.load(Ordering::Relaxed){return Ok("Download paused".into());}std::thread::sleep(Duration::from_millis(100));}
                    continue 'transfer;
                }
                return Err(format!("Download interrupted ({status}); partial file preserved. Start again to resume."));
            }
            break 'transfer;
        }
        report(format!(
            "Downloaded: {:.1} MiB",
            fs::metadata(&part).map(|m| m.len()).unwrap_or(0) as f64 / 1048576.0
        ));
        std::thread::sleep(Duration::from_millis(500));
    }
    }
    report("SHA-256 controleren...".into());
    verify_download(&part, &target, expected)?;
    let _ = fs::remove_file(meta);
    atomic(&dir.join(format!("{name}.verified.json")), &source)?;
    Ok(format!(
        "Geverifieerd: {}. Stel --model/--mmproj in bij Configuration.",
        target.display()
    ))
}
fn verify_download(part: &Path, target: &Path, expected: &str) -> Result<()> {
    let mut input = File::open(part).map_err(err)?;
    let mut header = [0; 4];
    input.read_exact(&mut header).map_err(err)?;
    if &header != b"GGUF" {
        return Err("Not a GGUF file; not activated".into());
    }
    let mut digest = Sha256::new();
    digest.update(header);
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let n = input.read(&mut buffer).map_err(err)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    drop(input);
    if format!("{:x}", digest.finalize()) != expected.to_lowercase() {
        fs::rename(part, part.with_extension(format!("rejected-{}", id()))).map_err(err)?;
        return Err("SHA-256 wijkt af; download apart gezet".into());
    }
    if target.exists() {
        return Err("Doelbestand bestaat al".into());
    }
    fs::rename(part, target).map_err(err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn usb_paths_follow_root() {
        let cfg=json!({"paths":{"data":"data","logs":"logs","models":"models","backups":"backups"},"services":{"qwen3":{"port":8080,"command":"runtime/llamafile.exe","args":["--model","models/qwen.gguf"]}}});
        validate(&cfg).unwrap();
        let resolved=resolve_config(Path::new("F:/"),cfg.clone());
        assert_eq!(path(&resolved,"data"),PathBuf::from("F:/data"));
        assert_eq!(portable_config(&resolved),cfg);
    }
    #[test]
    fn backup_and_conflict() {
        let root = root().unwrap().join("tests").join(format!("ai-usb-rust-test-{}", id()));
        let cfg = json!({"paths":{"data":root.join("data"),"backups":root.join("backups"),"models":root.join("models"),"logs":root.join("logs")},"services":{}});
        let mut state = load_chats(&cfg).unwrap();
        state["chats"] = json!([{"id":"test","messages":[]}]);
        let mut stale = state.clone();
        save_chats(&cfg, &mut state).unwrap();
        assert!(save_chats(&cfg, &mut stale).is_err());
        let archive = backup(&cfg).unwrap();
        state["chats"] = json!([]);
        save_chats(&cfg, &mut state).unwrap();
        restore(&cfg, &root, &archive).unwrap();
        assert_eq!(load_chats(&cfg).unwrap()["chats"][0]["id"], "test");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn streaming_preserves_chunks_and_detects_truncation(){
        for complete in [true,false]{
            let server=tiny_http::Server::http("127.0.0.1:0").unwrap();let port=server.server_addr().to_ip().unwrap().port();
            let thread=std::thread::spawn(move||{let mut request=server.recv().unwrap();let mut body=String::new();request.as_reader().read_to_string(&mut body).unwrap();assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["stream"],true);let mut stream="data: {\"choices\":[{\"delta\":{\"content\":\"Hallo \"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"wereld\"}}]}\n\n".to_string();if complete{stream.push_str("data: [DONE]\n\n");}request.respond(tiny_http::Response::from_string(stream)).unwrap();});
            let cfg=json!({"services":{"test":{"port":port}}});let mut chunks=vec![];let response=stream_response(&cfg,"test",json!({"messages":[]}),|event|{chunks.push(event);Ok(())});thread.join().unwrap();assert_eq!(chunks.len(),2);if complete{assert_eq!(response.unwrap()["choices"][0]["message"]["content"],"Hallo wereld");}else{assert!(response.is_err());}
        }
    }
    #[test]
    #[ignore]
    fn live_qwen_chat() {
        let cfg = config(&root().unwrap()).unwrap();
        let answer = chat(
            &cfg,
            "qwen3",
            json!([{"role":"user","content":"What is 2+2? Return only the number."}]),
        )
        .unwrap();
        assert_eq!(answer.trim(), "4");
    }
    #[test]
    fn hash_verification_never_installs_bad_download() {
        let dir = root().unwrap().join("tests").join(format!("ai-usb-hash-{}", id()));
        fs::create_dir_all(&dir).unwrap();
        let part = dir.join("model.part");
        let target = dir.join("model.gguf");
        let data = b"GGUFtestdata";
        fs::write(&part, data).unwrap();
        assert!(verify_download(&part, &target, &"0".repeat(64)).is_err());
        assert!(!target.exists());
        fs::write(&part, data).unwrap();
        let hash = format!("{:x}", Sha256::digest(data));
        verify_download(&part, &target, &hash).unwrap();
        assert_eq!(fs::read(&target).unwrap(), data);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn native_service_lifecycle() {
        let root = root().unwrap().join("tests").join(format!("ai-usb-process-{}", id()));
        let cfg = json!({"paths":{"data":root.join("data"),"backups":root.join("backups"),"models":root.join("models"),"logs":root.join("logs")},
            "services":{"test":{"port":59998,"command":"ping.exe","args":["-t","127.0.0.1"]}}});
        atomic(&root.join("config/settings.json"), &cfg).unwrap();
        service(&root, "test", true).unwrap();
        let first = read_json(&root.join("data/processes/test.rust.json")).unwrap();
        service(&root, "test", true).unwrap();
        assert_eq!(
            read_json(&root.join("data/processes/test.rust.json")).unwrap(),
            first
        );
        service(&root, "test", false).unwrap();
        assert!(identity(first["pid"].as_u64().unwrap() as u32).is_none());
        fs::remove_dir_all(root).unwrap();
    }
}
