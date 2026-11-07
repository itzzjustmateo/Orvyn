use crate::core::{self, err, Result};
use serde_json::{json, Value};
use std::{fs, io::Write, path::Path, process::Stdio};
use base64::Engine;
use sha2::{Digest,Sha256};

fn powershell(script: &str, input: &Value) -> Result<Value> {
    let mut child=core::hidden("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",script]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(err)?;
    child.stdin.take().ok_or("Input is missing")?.write_all(input.to_string().as_bytes()).map_err(err)?;
    let out=child.wait_with_output().map_err(err)?;
    if !out.status.success(){return Err(String::from_utf8_lossy(&out.stderr).to_string());}
    serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim_start_matches('\u{feff}')).map_err(err)
}
pub fn hardware()->Result<Value>{
    powershell("$ErrorActionPreference='Stop'; [Console]::OutputEncoding=[Text.Encoding]::UTF8; $o=Get-CimInstance Win32_OperatingSystem; $c=@(Get-CimInstance Win32_Processor); $p=@(Get-Process | Where-Object {$_.ProcessName -like '*llamafile*'}); @{ram_bytes=[double]$o.TotalVisibleMemorySize*1024;available_bytes=[double]$o.FreePhysicalMemory*1024;cpu=($c.Name -join ', ');threads=($c | Measure-Object NumberOfLogicalProcessors -Sum).Sum;model_ram_bytes=($p | Measure-Object WorkingSet64 -Sum).Sum} | ConvertTo-Json -Compress",&Value::Null)
}
fn size(path:&Path)->u64{
    let Ok(meta)=fs::symlink_metadata(path) else{return 0};
    if meta.file_type().is_symlink(){return 0;}
    if meta.is_file(){return meta.len();}
    fs::read_dir(path).ok().into_iter().flatten().filter_map(|e|e.ok()).map(|e|size(&e.path())).sum()
}
pub fn storage(root:&Path)->Value{
    json!({"models":size(&root.join("models")),"conversations":size(&root.join("data/chats.json")),"documents":size(&root.join("data/documents")),"backups":size(&root.join("backups")),"profile":size(&root.join("data/webview-profile"))})
}
pub fn import_document(root:&Path,data:&Value)->Result<Value>{
    let name=data["name"].as_str().ok_or("Filename is missing")?;
    let text=data["text"].as_str().ok_or("Text is missing")?;
    if name.len()>240||text.len()>2_000_000||text.trim().is_empty(){return Err("Document is empty or too large (maximum 2 MB of text)".into());}
    let id=if let Some(source)=data["source"].as_str(){format!("index-{:x}",Sha256::digest(source.as_bytes()))}else{core::id()};let dir=root.join("data/documents");fs::create_dir_all(&dir).map_err(err)?;
    if let Some(encoded)=data["pdf"].as_str(){let bytes=base64::engine::general_purpose::STANDARD.decode(encoded).map_err(err)?;if bytes.len()>20_000_000||!bytes.starts_with(b"%PDF"){return Err("Invalid PDF".into());}fs::write(dir.join(format!("{id}.pdf")),bytes).map_err(err)?;}
    core::atomic(&dir.join(format!("{id}.json")),&json!({"id":id,"name":name,"text":text,"collection":data["collection"].as_str().unwrap_or("Algemeen"),"source":data["source"],"fingerprint":data["fingerprint"]}))?;
    Ok(json!({"id":id,"name":name}))
}
pub fn documents(root:&Path)->Result<Value>{
    let dir=root.join("data/documents");let mut result=vec![];
    if dir.exists(){for e in fs::read_dir(dir).map_err(err)?{let p=e.map_err(err)?.path();if p.extension().is_some_and(|e|e=="json"){if let Ok(d)=core::read_json(&p){if d["missing"]!=true{result.push(json!({"id":d["id"],"name":d["name"],"collection":d["collection"],"source":d["source"]}));}}}}}
    Ok(json!(result))
}
pub fn search(root:&Path,data:&Value)->Result<Value>{
    let query=data["query"].as_str().unwrap_or("").to_lowercase();
    let mut terms=query.clone();if let Some(expansion)=data["terms"].as_str(){terms.push(' ');terms.push_str(expansion);}
    let words:Vec<_>=terms.split_whitespace().filter(|w|w.len()>2).collect();
    let selected=data["id"].as_str();let mut hits=vec![];
    let dir=root.join("data/documents");if !dir.exists(){return Ok(json!(hits));}
    for e in fs::read_dir(&dir).map_err(err)?{
        let p=e.map_err(err)?.path();if p.extension().is_none_or(|e|e!="json"){continue;}
        let d=core::read_json(&p)?;if selected.is_some_and(|id|d["id"]!=id){continue;}
        if d["missing"]==true{continue;}
        if data["collection"].as_str().is_some_and(|c|!c.is_empty()&&d["collection"]!=c){continue;}
        let text=d["text"].as_str().unwrap_or("");
        let pdf=dir.join(format!("{}.pdf",d["id"].as_str().unwrap_or(""))).exists();let mut section=0;
        let segments:Vec<&str>=if pdf{text.split("[Page ").collect()}else{vec![text]};
        for (i,segment) in segments.iter().enumerate(){let (page,body)=if pdf&&i>0{let (n,body)=segment.split_once(']').unwrap_or(("1",segment));(n.parse::<u64>().unwrap_or(1),body)}else{(1,*segment)};
            let chars:Vec<char>=body.chars().collect();for chunk in chars.chunks(1200){let content:String=chunk.iter().collect();section+=1;let lower=content.to_lowercase();let score=words.iter().filter(|w|lower.contains(**w)).count();if score>0||selected.is_some(){hits.push(json!({"name":d["name"],"id":d["id"],"section":section,"page":page,"pdf":pdf,"text":content,"score":score}));}}
        }
    }
    hits.sort_by_key(|h|std::cmp::Reverse(h["score"].as_u64().unwrap_or(0)));hits.truncate(4);Ok(json!(hits))
}
pub fn document(root:&Path,data:&Value)->Result<Value>{
    let id=data["id"].as_str().ok_or("Document is missing")?;if id.is_empty()||!id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'){return Err("Invalid document ID".into());}
    let mut doc=core::read_json(&root.join("data/documents").join(format!("{id}.json")))?;
    if data["pdf"]==true{let path=root.join("data/documents").join(format!("{id}.pdf"));if path.exists(){doc["pdf"]=json!(base64::engine::general_purpose::STANDARD.encode(fs::read(path).map_err(err)?));}}
    Ok(doc)
}
pub fn export_code(root:&Path,data:&Value)->Result<Value>{
    let name=data["name"].as_str().ok_or("Filename is missing")?;
    let text=data["text"].as_str().ok_or("Code is missing")?;
    let extension=Path::new(name).extension().and_then(|e|e.to_str()).unwrap_or("").to_lowercase();
    let stem=name.split('.').next().unwrap_or("").to_uppercase();
    if name.len()>120||name.starts_with('.')||!name.bytes().all(|b|b.is_ascii_alphanumeric()||b"._-".contains(&b))||
       ["CON","PRN","AUX","NUL","COM1","COM2","COM3","COM4","COM5","COM6","COM7","COM8","COM9","LPT1","LPT2","LPT3","LPT4","LPT5","LPT6","LPT7","LPT8","LPT9"].contains(&stem.as_str())||
       !["ts","tsx","js","jsx","py","rs","go","java","cpp","h","c","md","css","html","json","yaml","yml","toml","sql","txt"].contains(&extension.as_str()){
        return Err("Choose a supported code filename without folders".into());
    }
    if text.len()>2_000_000{return Err("Code files must be no larger than 2 MB".into());}
    let directory=root.join("data/exports").join(format!("opex-{}",core::id()));
    fs::create_dir_all(&directory).map_err(err)?;
    let path=directory.join(name);fs::write(&path,text).map_err(err)?;
    Ok(json!({"path":path}))
}
pub fn export(root:&Path,data:&Value)->Result<Value>{
    let extension=data["extension"].as_str().unwrap_or("txt");if !["txt","md","pdf"].contains(&extension){return Err("Invalid export format".into());}
    let bytes=if extension=="pdf"{base64::engine::general_purpose::STANDARD.decode(data["base64"].as_str().ok_or("PDF is missing")?).map_err(err)?}else{data["text"].as_str().ok_or("Text is missing")?.as_bytes().to_vec()};
    if bytes.len()>20_000_000{return Err("Export te groot".into());}
    let dir=root.join("data/exports");fs::create_dir_all(&dir).map_err(err)?;let path=dir.join(format!("export-{}.{}",core::id(),extension));fs::write(&path,bytes).map_err(err)?;Ok(json!({"path":path}))
}
pub fn index_folder(root:&Path,data:&Value)->Result<Value>{
    let relative=data["folder"].as_str().ok_or("Folder is missing")?;
    let file=root.join("data/index-folders.json");let mut list=if file.exists(){core::read_json(&file)?}else{json!([])};
    let paths=list.as_array_mut().ok_or("Invalid folder index")?;
    // Removing a saved entry must also work after its directory has disappeared.
    if data["remove"]==true{
        paths.retain(|p|p["folder"].as_str().is_none_or(|saved|Path::new(saved)!=Path::new(relative)));
    }else{
        let path=root.join(relative).canonicalize().map_err(err)?;let base=root.canonicalize().map_err(err)?;
        if !path.starts_with(&base)||!path.is_dir()||path==base{return Err("Choose an existing subfolder on this USB".into());}
        let relative=path.strip_prefix(&base).map_err(err)?.to_string_lossy().to_string();
        if !paths.iter().any(|p|p["folder"]==relative){paths.push(json!({"folder":relative,"collection":data["collection"].as_str().unwrap_or("Algemeen")}));}
    }
    core::atomic(&file,&list)?;Ok(list)
}
pub fn scan_index(root:&Path)->Result<Value>{
    let file=root.join("data/index-folders.json");if !file.exists(){return Ok(json!({"folders":[],"files":[]}));}
    let folders=core::read_json(&file)?;let mut files=vec![];let mut bytes=0;
    let documents=root.join("data/documents");if documents.exists(){for e in fs::read_dir(&documents).map_err(err)?{let path=e.map_err(err)?.path();if path.extension().is_some_and(|e|e=="json"){if let Ok(mut d)=core::read_json(&path){if let Some(source)=d["source"].as_str(){let missing=!root.join(source).is_file();if d["missing"].as_bool().unwrap_or(false)!=missing{d["missing"]=json!(missing);core::atomic(&path,&d)?;}}}}}}
    fn walk(root:&Path,dir:&Path,collection:&Value,files:&mut Vec<Value>,bytes:&mut u64,depth:u32)->Result<()>{
        if depth>12||files.len()>=20||*bytes>=20_000_000{return Ok(());}
        for entry in fs::read_dir(dir).map_err(err)?{
            if files.len()>=20||*bytes>=20_000_000{break;}
            let entry=entry.map_err(err)?;let path=entry.path();let meta=fs::symlink_metadata(&path).map_err(err)?;if meta.file_type().is_symlink(){continue;}
            let name=entry.file_name().to_string_lossy().to_string();if name.starts_with('.')||["node_modules","target","build-usb","models","archive","backups","webview-profile"].contains(&name.as_str()){continue;}
            if meta.is_dir(){walk(root,&path,collection,files,bytes,depth+1)?;continue;}
            let ext=path.extension().and_then(|e|e.to_str()).unwrap_or("").to_lowercase();if !["txt","md","csv","json","py","js","ts","tsx","rs","html","css","pdf","docx","xlsx"].contains(&ext.as_str())||meta.len()>10_000_000{continue;}
            let source=path.strip_prefix(root).map_err(err)?.to_string_lossy().to_string();if files.iter().any(|f|f["source"]==source)||*bytes+meta.len()>20_000_000{continue;}let id=format!("index-{:x}",Sha256::digest(source.as_bytes()));let content=fs::read(&path).map_err(err)?;let fingerprint=format!("{:x}",Sha256::digest(&content));
            if core::read_json(&root.join("data/documents").join(format!("{id}.json"))).is_ok_and(|d|d["fingerprint"]==fingerprint){continue;}
            *bytes+=meta.len();files.push(json!({"name":name,"source":source,"collection":collection,"fingerprint":fingerprint,"base64":base64::engine::general_purpose::STANDARD.encode(content)}));if files.len()>=20||*bytes>=20_000_000{break;}
        }Ok(())
    }
    for f in folders.as_array().ok_or("Invalid folder index")?{if let Some(folder)=f["folder"].as_str(){let path=root.join(folder);if path.canonicalize().is_ok_and(|p|p.starts_with(root.canonicalize().unwrap_or_default())){walk(root,&path,&f["collection"],&mut files,&mut bytes,0)?;}}}
    Ok(json!({"folders":folders,"files":files}))
}
fn encode(text:&str)->String{text.bytes().map(|b|if b.is_ascii_alphanumeric()||b"-_.~".contains(&b){(b as char).to_string()}else{format!("%{b:02X}")}).collect()}
pub fn web_search(data:&Value)->Result<Value>{
    let query=data["query"].as_str().ok_or("Search query is missing")?;
    let url=format!("https://en.wikipedia.org/w/api.php?action=query&generator=search&gsrsearch={}&gsrlimit=4&prop=extracts|info&inprop=url&exintro=1&explaintext=1&exchars=1400&format=json",encode(query));
    let out=core::hidden("curl.exe").args(["-fsSL","--max-time","25","-A","AI-USB/0.6 (personal desktop assistant)",&url]).output().map_err(err)?;
    if !out.status.success(){return Err("Online search failed".into());}
    let data:Value=serde_json::from_slice(&out.stdout).map_err(err)?;
    let hits:Vec<Value>=data["query"]["pages"].as_object().into_iter().flat_map(|p|p.values()).map(|p|json!({"name":p["title"],"url":p["fullurl"],"text":p["extract"]})).collect();Ok(json!(hits))
}
pub fn speech(data:&Value)->Result<Value>{
    static SPEECH:std::sync::Mutex<()>=std::sync::Mutex::new(());
    let _guard=SPEECH.try_lock().map_err(|_|"A speech request is already running")?;
    if data["action"]=="listen"{
        powershell("$ErrorActionPreference='Stop'; [Console]::OutputEncoding=[Text.Encoding]::UTF8; $d=[Console]::In.ReadToEnd()|ConvertFrom-Json; Add-Type -AssemblyName System.Speech; $info=[System.Speech.Recognition.SpeechRecognitionEngine]::InstalledRecognizers()|Where-Object {$_.Culture.Name -eq $d.language}|Select-Object -First 1; if(!$info){throw 'No offline Windows speech recognizer is installed for the selected language.'}; $r=New-Object System.Speech.Recognition.SpeechRecognitionEngine($info); try {$r.LoadGrammar((New-Object System.Speech.Recognition.DictationGrammar)); $r.SetInputToDefaultAudioDevice(); $v=$r.Recognize([TimeSpan]::FromSeconds(15)); @{text=$v.Text;language=$r.RecognizerInfo.Culture.Name}|ConvertTo-Json -Compress} finally {$r.Dispose()}",data)
    }else{
        if data["text"].as_str().is_none_or(|text|text.len()>40_000){return Err("Text to read aloud is missing or too long".into());}
        powershell("$ErrorActionPreference='Stop'; [Console]::InputEncoding=[Text.Encoding]::UTF8; $d=[Console]::In.ReadToEnd()|ConvertFrom-Json; Add-Type -AssemblyName System.Speech; $s=New-Object System.Speech.Synthesis.SpeechSynthesizer; try {$v=$s.GetInstalledVoices()|Where-Object {$_.VoiceInfo.Culture.Name -like 'nl-*'}|Select-Object -First 1; if($v){$s.SelectVoice($v.VoiceInfo.Name)}; $s.Speak([string]$d.text); '{\"ok\":true}'} finally {$s.Dispose()}",data)
    }
}
pub fn open_url(data:&Value)->Result<Value>{
    let url=data["url"].as_str().ok_or("URL is missing")?;
    crate::web_pages::validate(url)?;
    powershell("$ErrorActionPreference='Stop'; $d=[Console]::In.ReadToEnd()|ConvertFrom-Json; Start-Process ([string]$d.url); '{\"ok\":true}'",data)
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn index_tracks_changes_with_usb_relative_paths(){
        let root=core::root().unwrap().join("tools/temp").join(format!("index-{}",core::id()));fs::create_dir_all(root.join("source")).unwrap();fs::write(root.join("source/a.txt"),"A car drives on the road.").unwrap();
        assert!(index_folder(&root,&json!({"folder":".."})).is_err());
        let folders=index_folder(&root,&json!({"folder":"source","collection":"Traffic"})).unwrap();assert_eq!(folders[0]["folder"],"source");
        let scan=scan_index(&root).unwrap();let mut doc=scan["files"][0].clone();assert_eq!(doc["source"],"source\\a.txt");doc["text"]=json!("A car drives on the road.");import_document(&root,&doc).unwrap();
        assert_eq!(scan_index(&root).unwrap()["files"].as_array().unwrap().len(),0);
        assert_eq!(search(&root,&json!({"query":"vehicle","terms":"car","collection":"Traffic"})).unwrap().as_array().unwrap().len(),1);
        assert_eq!(search(&root,&json!({"query":"car","collection":"School"})).unwrap().as_array().unwrap().len(),0);
        fs::write(root.join("source/a.txt"),"An updated text").unwrap();assert_eq!(scan_index(&root).unwrap()["files"].as_array().unwrap().len(),1);
        fs::remove_file(root.join("source/a.txt")).unwrap();scan_index(&root).unwrap();assert_eq!(documents(&root).unwrap().as_array().unwrap().len(),0);fs::remove_dir_all(root).unwrap();
    }
    #[test]fn pdf_sources_keep_page_numbers(){
        let root=core::root().unwrap().join("tools/temp").join(format!("pages-{}",core::id()));
        let d=import_document(&root,&json!({"name":"test.pdf","text":"[Page 1]\nEerste pagina.\n[Page 2]\nSpecifiek zoekwoord.","pdf":base64::engine::general_purpose::STANDARD.encode(b"%PDF-test")})).unwrap();
        let hits=search(&root,&json!({"query":"Specifiek"})).unwrap();assert_eq!(hits[0]["page"],2);assert_eq!(hits[0]["pdf"],true);
        assert!(document(&root,&json!({"id":"../../settings"})).is_err());assert!(document(&root,&json!({"id":d["id"],"pdf":true})).unwrap()["pdf"].is_string());fs::remove_dir_all(root).unwrap();
    }
    #[test]fn removed_index_folder_can_be_unregistered(){
        let root=core::root().unwrap().join("tools/temp").join(format!("removed-folder-{}",core::id()));
        fs::create_dir_all(root.join("source")).unwrap();
        index_folder(&root,&json!({"folder":"source"})).unwrap();
        fs::remove_dir(root.join("source")).unwrap();
        assert_eq!(index_folder(&root,&json!({"folder":"source","remove":true})).unwrap(),json!([]));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]fn scan_limits_apply_across_nested_folders(){
        let root=core::root().unwrap().join("tools/temp").join(format!("scan-limit-{}",core::id()));
        fs::create_dir_all(root.join("source/nested")).unwrap();
        for i in 0..25{fs::write(root.join(format!("source/nested/{i}.txt")),"nested").unwrap();fs::write(root.join(format!("source/{i}.txt")),"parent").unwrap();}
        index_folder(&root,&json!({"folder":"source"})).unwrap();
        assert_eq!(scan_index(&root).unwrap()["files"].as_array().unwrap().len(),20);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]fn overlapping_index_folders_do_not_duplicate_files(){
        let root=core::root().unwrap().join("tools/temp").join(format!("scan-duplicate-{}",core::id()));
        fs::create_dir_all(root.join("source/nested")).unwrap();
        fs::write(root.join("source/nested/a.txt"),"hello").unwrap();
        index_folder(&root,&json!({"folder":"source"})).unwrap();
        index_folder(&root,&json!({"folder":"source/nested"})).unwrap();
        assert_eq!(scan_index(&root).unwrap()["files"].as_array().unwrap().len(),1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]fn scan_respects_byte_limit(){
        let root=core::root().unwrap().join("tools/temp").join(format!("scan-bytes-{}",core::id()));
        fs::create_dir_all(root.join("source")).unwrap();
        for i in 0..3{fs::write(root.join(format!("source/{i}.txt")),vec![b'a';7_000_000]).unwrap();}
        index_folder(&root,&json!({"folder":"source"})).unwrap();
        assert_eq!(scan_index(&root).unwrap()["files"].as_array().unwrap().len(),2);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]fn opex_exports_new_files_without_overwriting_sources(){
        let root=core::root().unwrap().join("tests").join(format!("opex-{}",core::id()));
        fs::create_dir_all(&root).unwrap();fs::write(root.join("example.py"),"original").unwrap();
        let a=export_code(&root,&json!({"name":"example.py","text":"print('hé')"})).unwrap();
        let b=export_code(&root,&json!({"name":"example.py","text":"new"})).unwrap();
        assert_ne!(a["path"],b["path"]);assert_eq!(fs::read_to_string(a["path"].as_str().unwrap()).unwrap(),"print('hé')");
        assert_eq!(fs::read_to_string(root.join("example.py")).unwrap(),"original");
        for name in ["../example.py","CON.py","run.exe",".env","a/b.py"]{assert!(export_code(&root,&json!({"name":name,"text":"x"})).is_err());}
        fs::remove_dir_all(root).unwrap();
    }
    #[test]fn url_query_is_encoded(){assert_eq!(encode("a &b/é"),"a%20%26b%2F%C3%A9");}
    #[test]fn hardware_reports_memory(){let h=hardware().unwrap();assert!(h["ram_bytes"].as_f64().unwrap()>0.0);assert!(h["threads"].as_f64().unwrap()>0.0);assert!(h["cpu"].is_string());}
    #[test]fn document_search_returns_source(){let root=core::root().unwrap().join("tools/temp").join(format!("documents-{}",core::id()));fs::create_dir_all(&root).unwrap();let d=import_document(&root,&json!({"name":"geschiedenis.txt","text":"De Tweede Wereldoorlog begon in 1939."})).unwrap();let hits=search(&root,&json!({"query":"Wereldoorlog"})).unwrap();assert_eq!(hits[0]["id"],d["id"]);assert_eq!(hits[0]["section"],1);assert!(import_document(&root,&json!({"name":"leeg","text":" "})).is_err());fs::remove_dir_all(root).unwrap();}
}
