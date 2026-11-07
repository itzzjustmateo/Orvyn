use crate::core::{self,err,Result};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{fs::{self,File},io::{Read,Write},path::Path,sync::atomic::{AtomicBool,Ordering}};

pub fn validate(name:&str,category:&str)->Result<()> {
    if name.trim().is_empty()||name.chars().count()>80{return Err("Enter a model name of up to 80 characters".into());}
    if !["chat","code","reasoning","vision"].contains(&category){return Err("Invalid model category".into());}
    Ok(())
}
pub fn append(root:&Path,entry:Value)->Result<()> {
    let _lock=core::Lock::acquire("Local\\AIUSB-CustomCatalog")?;
    let file=root.join("config/custom-models.json");
    let mut data=if file.exists(){core::read_json(&file)?}else{json!({"version":1,"models":[]})};
    data["models"].as_array_mut().ok_or("Invalid custom model catalog")?.push(entry);
    core::atomic(&file,&data)
}
pub fn remote(root:&Path,data:&Value)->Result<String>{
    let name=data["name"].as_str().unwrap_or("").trim();let category=data["category"].as_str().unwrap_or("chat");validate(name,category)?;
    if category=="vision"{return Err("Use local import to add a vision model with a projector".into());}
    let url=data["url"].as_str().unwrap_or("").trim();let sha=data["sha256"].as_str().unwrap_or("").trim().to_lowercase();
    if !url.starts_with("https://")||sha.len()!=64||!sha.bytes().all(|b|b.is_ascii_hexdigit()){return Err("Enter an HTTPS download link and a valid publisher SHA-256".into());}
    let id=format!("custom-{}",core::id());
    append(root,json!({"id":id,"name":name,"category":category,"description":"Eigen model via downloadlink","origin":"custom","file":format!("{id}.gguf"),"url":url,"sha256":sha,"bytes":0,"system_role":data["system_role"].as_bool().unwrap_or(true)}))?;
    Ok(id)
}
pub fn copy_gguf(source:&Path,target:&Path,cancel:&AtomicBool,progress:impl Fn(u64))->Result<Value>{
    let mut input=File::open(source).map_err(err)?;
    let total=input.metadata().map_err(err)?.len();
    let mut magic=[0;4];input.read_exact(&mut magic).map_err(err)?;
    if &magic!=b"GGUF"{return Err("The selected file is not a GGUF model".into());}
    if target.exists(){return Err("Doelbestand bestaat al".into());}
    fs::create_dir_all(target.parent().ok_or("Destination folder is missing")?).map_err(err)?;
    let temp=target.with_extension("gguf.importing");
    let outcome=(||{
        let mut output=File::create(&temp).map_err(err)?;let mut hash=Sha256::new();hash.update(magic);output.write_all(&magic).map_err(err)?;
        let mut copied=4;let mut buffer=vec![0;1024*1024];
        loop{if cancel.load(Ordering::Relaxed){return Err("Import canceled; the original file was preserved".into());}
            let count=input.read(&mut buffer).map_err(err)?;if count==0{break;}
            output.write_all(&buffer[..count]).map_err(err)?;hash.update(&buffer[..count]);copied+=count as u64;progress(copied);
        }
        output.sync_all().map_err(err)?;drop(output);
        if copied!=total{return Err("The source file changed while copying".into());}
        let sha256=format!("{:x}",hash.finalize());
        fs::rename(&temp,target).map_err(err)?;
        core::atomic(&target.with_extension("gguf.verified.json"),&json!({"origin":"local","sha256":sha256,"bytes":copied}))?;
        Ok(json!({"file":target.file_name().unwrap().to_string_lossy(),"sha256":sha256,"bytes":copied,"origin":"local"}))
    })();
    if outcome.is_err(){let _=fs::remove_file(temp);}
    outcome
}
#[cfg(test)] mod tests {
    use super::*;
    #[test]fn local_import_preserves_source_and_checks_header(){
        let root=core::root().unwrap().join("tests").join(format!("custom-import-{}",core::id()));fs::create_dir_all(&root).unwrap();
        let source=root.join("source.gguf");let target=root.join("models/copy.gguf");let bytes=b"GGUFexamplepayload";fs::write(&source,bytes).unwrap();
        let info=copy_gguf(&source,&target,&AtomicBool::new(false),|_|{}).unwrap();
        assert_eq!(info["sha256"],format!("{:x}",Sha256::digest(bytes)));assert_eq!(fs::read(&source).unwrap(),fs::read(&target).unwrap());
        fs::write(&source,b"not a model").unwrap();assert!(copy_gguf(&source,&root.join("bad.gguf"),&AtomicBool::new(false),|_|{}).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]fn url_requires_sha(){
        let root=core::root().unwrap().join("tests").join(format!("custom-url-{}",core::id()));
        assert!(validate("","chat").is_err());assert!(validate("Eigen model","unknown").is_err());
        assert!(remote(&root,&json!({"name":"Eigen model","category":"chat","url":"https://example.com/model.gguf","sha256":"wrong"})).is_err());
        assert!(!root.exists());
        let id=remote(&root,&json!({"name":"Eigen model","category":"chat","url":"https://example.com/model.gguf","sha256":"0".repeat(64),"system_role":false})).unwrap();
        let saved=core::read_json(&root.join("config/custom-models.json")).unwrap();
        assert_eq!(saved["models"][0]["id"],id);assert_eq!(saved["models"][0]["system_role"],false);
        fs::remove_dir_all(root).unwrap();
    }
}
