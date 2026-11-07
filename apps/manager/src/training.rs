use crate::core::{self,err,Result};
use serde_json::{json,Value};
use std::{collections::HashSet,fs,path::{Path,PathBuf}};

fn read_small(path:&Path)->Result<Value>{
    if fs::metadata(path).map_err(err)?.len()>2_000_000{return Err("Trainingsrapport is groter dan 2 MB.".into());}
    core::read_json(path)
}
fn optional(path:&Path)->Value{if !path.exists(){Value::Null}else{read_small(path).unwrap_or_else(|e|json!({"error":e}))}}
fn dataset(root:&Path)->PathBuf{
    let seed=root.join("data/training/orvyn-1");let v2=seed.join("v2");if v2.is_dir(){v2}else{seed}
}
fn rows(root:&Path,split:&str)->Result<Vec<Value>>{
    if !["train","eval"].contains(&split){return Err("Choose train or eval.".into());}
    let path=dataset(root).join(format!("{split}.jsonl"));
    if !path.exists(){return Ok(vec![]);}
    if fs::metadata(&path).map_err(err)?.len()>2_000_000{return Err("Trainingsset is groter dan 2 MB.".into());}
    fs::read_to_string(path).map_err(err)?.trim_start_matches('\u{feff}').lines().filter(|s|!s.trim().is_empty()).enumerate()
        .map(|(i,line)|serde_json::from_str(line).map_err(|e|format!("{split}, regel {}: {e}",i+1))).collect()
}
pub fn examples(root:&Path,input:&Value)->Result<Value>{
    let split=input["split"].as_str().unwrap_or("train");let domain=input["domain"].as_str().unwrap_or("nl");
    if !["nl","code","web","pc"].contains(&domain){return Err("Onbekend trainingsonderwerp.".into());}
    Ok(json!({"examples":rows(root,split)?.into_iter().filter(|r|r["domain"]==domain).take(100).collect::<Vec<_>>()}))
}
pub fn candidate(root:&Path,input:&Value)->Result<Value>{
    let id=input["id"].as_str().ok_or("Candidate is missing")?;
    if !id.starts_with("orvyn1-candidate-")||id.len()>100||!id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'){return Err("Invalid candidate ID.".into());}
    let parent=fs::canonicalize(root.join("models/training")).map_err(err)?;
    let dir=fs::canonicalize(parent.join(id)).map_err(err)?;
    if !dir.starts_with(&parent)||!dir.is_dir(){return Err("Candidate is outside the training folder.".into());}
    Ok(json!({"id":id,"evaluation":optional(&dir.join("evaluation.json")),"answers":optional(&dir.join("answers.json")),
        "review":optional(&dir.join("review.json")),"integrity":optional(&dir.join("integrity.json"))}))
}
pub fn overview(root:&Path)->Result<Value>{
    let mut candidates=vec![];
    if let Ok(entries)=fs::read_dir(root.join("models/training")){
        let mut ids=entries.filter_map(|e|e.ok()).filter(|e|e.file_type().is_ok_and(|t|t.is_dir())).map(|e|e.file_name().to_string_lossy().into_owned()).collect::<Vec<_>>();
        ids.sort();ids.reverse();
        for id in ids.into_iter().take(100){if let Ok(v)=candidate(root,&json!({"id":id})){candidates.push(json!({"id":id,"evaluation":v["evaluation"],"review":v["review"]}));}}
    }
    let mut counts=json!({"train":{"nl":0,"code":0,"web":0,"pc":0},"eval":{"nl":0,"code":0,"web":0,"pc":0}});
    let mut issues=vec![];let mut ids=HashSet::new();let mut prompts=HashSet::new();
    for split in ["train","eval"]{
        match rows(root,split){
            Err(e)=>issues.push(e),
            Ok(rows)=>{
                if rows.is_empty(){issues.push(format!("No examples in {split}."));}
                for row in rows{
                    let id=row["id"].as_str().unwrap_or("");let domain=row["domain"].as_str().unwrap_or("");
                    if id.is_empty()||!ids.insert(id.to_owned()){issues.push(format!("Ontbrekende of dubbele id: {id}"));}
                    if !["nl","code","web","pc"].contains(&domain){issues.push(format!("{id}: invalid topic"));continue;}
                    counts[split][domain]=json!(counts[split][domain].as_u64().unwrap_or(0)+1);
                    let messages=row["messages"].as_array();
                    if !messages.is_some_and(|m|m.len()==3&&["system","user","assistant"].iter().enumerate().all(|(i,role)|m[i]["role"]==*role&&m[i]["content"].as_str().is_some_and(|s|!s.trim().is_empty()))){issues.push(format!("{id}: invalid messages"));continue;}
                    let prompt=row["messages"][1]["content"].as_str().unwrap().split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
                    if !prompts.insert(prompt){issues.push(format!("{id}: duplicate question or overlap between training and evaluation"));}
                    if domain=="pc"{
                        let answer:Result<Value>=serde_json::from_str(row["messages"][2]["content"].as_str().unwrap()).map_err(err);
                        if answer.and_then(|v|crate::opex::proposal(&v)).is_err(){issues.push(format!("{id}: invalid Opex action"));}
                    }
                }
                for domain in ["nl","code","web","pc"]{if counts[split][domain]==0{issues.push(format!("{split}: no examples for {domain}"));}}
            }
        }
    }
    Ok(json!({"candidates":candidates,"progress":optional(&root.join("logs/training/progress.json")),
        "dataset":{"version":if dataset(root).ends_with("v2"){"v2"}else{"seed"},"counts":counts,"issues":issues,"manifest":optional(&dataset(root).join("manifest.json"))},
        "command":"tools\\training-venv\\Scripts\\python.exe tools\\training\\train_orvyn.py --train --device cpu --steps 24 --threads 4"}))
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn candidate_path_is_restricted(){for id in ["../x","orvyn1-candidate-../x","orvyn1-candidate-x/y","C:/outside"]{assert!(candidate(Path::new("."),&json!({"id":id})).is_err());}}
    #[test]fn empty_workspace_is_explicit(){let root=core::root().unwrap().join("tools/temp/nonexistent-training-fixture");let v=overview(&root).unwrap();assert_eq!(v["candidates"],json!([]));assert!(!v["dataset"]["issues"].as_array().unwrap().is_empty());}
}
