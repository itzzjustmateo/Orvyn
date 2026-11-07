//! Hugging Face model store: finds GGUF models through the API and adds them to the library.
//! Search results are saved as catalog entries so the existing verified
//! download queue, categories, and service settings also apply to store models.
use crate::core::{self, err, Result};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

const ENDPOINT: &str = "https://huggingface.co/api/models";
const AGENT: &str = "Orvyn/0.6 (model library)";
const LIMIT: usize = 12;
const MAX_STORE: usize = 40;
const MAX_FILE: u64 = 20_000_000_000;
const MAX_PROJECTOR: u64 = 5_000_000_000;
static SEARCHING: AtomicBool = AtomicBool::new(false);

fn get(url: &str, timeout: &str) -> Result<Value> {
    let output = core::hidden("curl.exe")
        .args([
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--connect-timeout",
            "10",
            "--max-time",
            timeout,
            "-A",
            AGENT,
            "--write-out",
            "\n%{http_code}",
            url,
        ])
        .output()
        .map_err(err)?;
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let (body, status) = match text.rsplit_once('\n') {
        Some(pair) => pair,
        None => (text.as_str(), "000"),
    };
    match status.trim() {
        "200" => serde_json::from_slice(body.as_bytes())
            .map_err(|_| "Hugging Face returned an unreadable response".into()),
        "429" | "503" | "504" => Err("Hugging Face is temporarily overloaded; try again shortly".into()),
        "401" | "403" | "404" => {
            Err("Hugging Face rejected this request or could not find the model".into())
        }
        _ => Err("Hugging Face is currently unreachable. Check your internet connection.".into()),
    }
}
fn encode(query: &str) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .append_pair("search", query)
        .finish()
}
fn hash(text: &str) -> u32 {
    let mut value: u32 = 0x811c9dc5;
    for byte in text.bytes() {
        value ^= byte as u32;
        value = value.wrapping_mul(0x01000193);
    }
    value
}
/// Stable service-safe ID for a Hugging Face repository.
fn slug(repo: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for character in repo.chars() {
        if character.is_ascii_alphanumeric() {
            out.push(character.to_ascii_lowercase());
            dash = false;
        } else if character.is_ascii() && !dash {
            out.push('-');
            dash = true;
        }
    }
    let mut out = out.trim_matches('-').to_string();
    if out.len() > 60 {
        out.truncate(60);
        out = out.trim_matches('-').to_string();
    }
    if out.is_empty() {
        out = "model".into();
    }
    format!("hf-{out}-{:06x}", hash(repo) & 0xffffff)
}
fn quant(file: &str) -> String {
    file.strip_suffix(".gguf")
        .unwrap_or(file)
        .rsplit('-')
        .next()
        .unwrap_or("")
        .to_uppercase()
}
/// USB model preference order: quality first, then file size.
fn score(lower: &str) -> i32 {
    for (token, value) in [
        ("q4_k_m", 100),
        ("q4_k_s", 96),
        ("q5_k_m", 92),
        ("q4_0", 88),
        ("q5_k_s", 84),
        ("q8_0", 80),
        ("iq4_xs", 76),
        ("q6_k", 74),
        ("q4_k_l", 72),
        ("q3_k_l", 64),
        ("q3_k_m", 60),
        ("iq3_m", 56),
        ("q3_0", 52),
        ("q2_k", 24),
    ] {
        if lower.contains(token) {
            // Q4_0 variants for edge devices are not standard Q4_0.
            if lower.contains("q4_0_4_4") || lower.contains("q4_0_4_8") || lower.contains("q4_0_8_8") {
                return value - 60;
            }
            return value;
        }
    }
    if lower.contains("q4") || lower.contains("q5") || lower.contains("q6") || lower.contains("q8") {
        return 48;
    }
    if lower.contains("q3") || lower.contains("iq3") {
        return 40;
    }
    if lower.contains("q2") || lower.contains("iq2") {
        return 16;
    }
    8
}
fn file_info(sibling: &Value) -> Option<(String, u64)> {
    let sha = sibling["lfs"]["sha256"].as_str()?.to_string();
    if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let bytes = sibling["lfs"]["size"]
        .as_u64()
        .or_else(|| sibling["size"].as_u64())
        .unwrap_or(0);
    if bytes == 0 {
        return None;
    }
    Some((sha, bytes))
}
fn pick(siblings: &[Value]) -> Option<(String, String, u64)> {
    let mut best: Option<(i32, String, String, u64)> = None;
    for sibling in siblings {
        let name = sibling["rfilename"].as_str().unwrap_or("");
        if !name.ends_with(".gguf") {
            continue;
        }
        let lower = name.to_lowercase();
        // Projectors and split files are not standalone models.
        if lower.contains("mmproj") || lower.contains("projector") || lower.contains("-of-") {
            continue;
        }
        let Some((sha, bytes)) = file_info(sibling) else {
            continue;
        };
        if bytes > MAX_FILE {
            continue;
        }
        let rank = score(&lower);
        let better = match &best {
            None => true,
            Some((old_rank, _, _, old_bytes)) => {
                rank > *old_rank || (rank == *old_rank && bytes < *old_bytes)
            }
        };
        if better {
            best = Some((rank, name.to_string(), sha, bytes));
        }
    }
    best.map(|(_, name, sha, bytes)| (name, sha, bytes))
}
fn pick_projector(siblings: &[Value]) -> Option<(String, String, u64)> {
    let mut best: Option<(u64, String, String)> = None;
    for sibling in siblings {
        let name = sibling["rfilename"].as_str().unwrap_or("");
        let lower = name.to_lowercase();
        if !name.ends_with(".gguf") || !(lower.contains("mmproj") || lower.contains("projector")) {
            continue;
        }
        let Some((sha, bytes)) = file_info(sibling) else {
            continue;
        };
        if bytes > MAX_PROJECTOR {
            continue;
        }
        if best.as_ref().is_none_or(|(old, _, _)| bytes < *old) {
            best = Some((bytes, name.to_string(), sha));
        }
    }
    best.map(|(bytes, name, sha)| (name, sha, bytes))
}
fn category(name: &str, tags: &[String], pipeline: &str) -> String {
    let mut hay = name.to_lowercase();
    for tag in tags {
        hay.push(' ');
        hay.push_str(&tag.to_lowercase());
    }
    let hay: String = hay
        .chars()
        .map(|c| if c == '-' || c == '_' { ' ' } else { c })
        .collect();
    let words: Vec<&str> = hay.split_whitespace().collect();
    // Short tokens such as "ocr" must match a whole word; "code" may also match "codellama".
    let inside = |tokens: &[&str]| {
        words
            .iter()
            .any(|word| tokens.iter().any(|token| word.contains(token)))
    };
    let word = |tokens: &[&str]| words.iter().any(|token| tokens.contains(token));
    if pipeline == "image-to-text"
        || pipeline == "image-text-to-text"
        || inside(&["vision", "llava", "moondream", "mmproj"])
        || word(&["ocr", "video", "image"])
    {
        return "vision".into();
    }
    if inside(&["code", "coder", "coding", "programming"]) {
        return "code".into();
    }
    if inside(&["reason", "math", "logic", "thinking"]) || word(&["r1"]) {
        return "reasoning".into();
    }
    "chat".into()
}
fn model_entry(repo: &str, detail: &Value, item: &Value) -> Option<Value> {
    let commit = detail["sha"].as_str()?;
    let siblings = detail["siblings"].as_array()?;
    let (file, sha, bytes) = pick(siblings)?;
    let name = repo.rsplit('/').next().unwrap_or(repo).to_string();
    let org = repo.split('/').next().unwrap_or("");
    let tags: Vec<String> = item["tags"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let category = category(&name, &tags, item["pipeline_tag"].as_str().unwrap_or(""));
    let downloads = item["downloads"].as_u64().unwrap_or(0);
    let id = slug(repo);
    let mut entry = json!({
        "id":id,
        "name":name,
        "category":category,
        "description":format!("{org} · GGUF {} · {:.2} GB · {downloads} downloads from Hugging Face",quant(&file),bytes as f64/1e9),
        "origin":"store",
        "store":true,
        "repo":repo,
        "file":format!("{id}.gguf"),
        "url":format!("https://huggingface.co/{repo}/resolve/{commit}/{file}"),
        "sha256":sha,
        "bytes":bytes,
        "downloads":downloads,
        "system_role":true
    });
    if category == "vision" {
        if let Some((projector, sha, bytes)) = pick_projector(siblings) {
            entry["projector"] = json!({
                "file":format!("{id}-mmproj.gguf"),
                "url":format!("https://huggingface.co/{repo}/resolve/{commit}/{projector}"),
                "sha256":sha,
                "bytes":bytes
            });
        }
    }
    Some(entry)
}
/// Saves new store models and removes the oldest uninstalled entries.
fn cap(models: &mut Vec<Value>, dir: &Path, protect: &[String]) {
    while models.len() > MAX_STORE {
        let index = models.iter().position(|entry| {
            let id = entry["id"].as_str().unwrap_or("");
            let file = entry["file"].as_str().unwrap_or("");
            !protect.iter().any(|busy| busy == id)
                && !file.is_empty()
                && !crate::models::downloaded_file_valid(&dir.join(file), entry)
        });
        match index {
            Some(index) => {
                models.remove(index);
            }
            None => break,
        }
    }
}
fn persist(root: &Path, entries: Vec<Value>, protect: &[String], loaded: bool) -> Result<usize> {
    let _lock = core::Lock::acquire("Local\\AIUSB-StoreCatalog")?;
    let file = root.join("config/store-models.json");
    let mut data = if file.exists() {
        core::read_json(&file)?
    } else {
        json!({"version":1,"models":[]})
    };
    let models = data["models"]
        .as_array_mut()
        .ok_or("Invalid store model catalog")?;
    let mut added = 0;
    for entry in entries {
        if !models.iter().any(|stored| stored["id"] == entry["id"]) {
            models.push(entry);
            added += 1;
        }
    }
    // Cleanup shifts later model ports; do it only when no model is loaded.
    if models.len() > MAX_STORE && !loaded {
        let cfg = core::config(root)?;
        let dir = core::path(&cfg, "models");
        cap(models, &dir, protect);
    }
    core::atomic(&file, &data)?;
    Ok(added)
}
fn run(root: &Path, data: &Value, protect: &[String], loaded: bool) -> Result<Value> {
    let query = data["query"].as_str().unwrap_or("").trim();
    if query.chars().count() > 60 || query.chars().any(|c| c.is_control()) {
        return Err("Invalid search term".into());
    }
    let mut url = format!("{ENDPOINT}?filter=gguf&sort=downloads&direction=-1&limit={LIMIT}");
    if !query.is_empty() {
        url.push('&');
        url.push_str(&encode(query));
    }
    let list = get(&url, "25")?;
    let items = list
        .as_array()
        .ok_or("Hugging Face returned an unreadable response")?;
    let mut seen: HashSet<String> = crate::models::catalog(root)?
        .iter()
        .filter_map(|entry| entry["id"].as_str().map(str::to_string))
        .collect();
    let mut jobs = vec![];
    for item in items.iter().take(LIMIT) {
        let repo = item["id"].as_str().unwrap_or("").to_string();
        if repo.is_empty() {
            continue;
        }
        let item = item.clone();
        jobs.push(std::thread::spawn(move || {
            let detail = get(&format!("{ENDPOINT}/{repo}?blobs=true"), "20").ok()?;
            model_entry(&repo, &detail, &item)
        }));
    }
    let mut entries = vec![];
    let mut added = 0;
    let mut matched = 0;
    for job in jobs {
        let Some(entry) = job.join().unwrap_or(None) else {
            continue;
        };
        let id = entry["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        if seen.insert(id) {
            added += 1;
            entries.push(entry);
        } else {
            matched += 1;
        }
    }
    let stored = persist(root, entries, protect, loaded)?;
    Ok(json!({"added":stored,"known":matched,"found":added+matched}))
}
/// Zoekt GGUF-modellen op Hugging Face en voegt nieuwe resultaten aan de bibliotheek toe.
/// `loaded` is true while a local model is running: preserve the ordering
/// so active service ports do not shift.
pub fn search(root: &Path, data: &Value, protect: &[String], loaded: bool) -> Result<Value> {
    if SEARCHING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A model search is already running".into());
    }
    let outcome = run(root, data, protect, loaded);
    SEARCHING.store(false, Ordering::SeqCst);
    outcome
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn slug_is_stable_and_safe_for_service_ids() {
        let id = slug("bartowski/Llama-3.2-1B-Instruct-GGUF");
        assert_eq!(id, slug("bartowski/Llama-3.2-1B-Instruct-GGUF"));
        assert!(id.starts_with("hf-bartowski-llama-3-2-1b-instruct-gguf-"));
        assert!(id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'));
        assert_ne!(id, slug("bartowski/Llama-3.2-3B-Instruct-GGUF"));
        assert!(slug("///").starts_with("hf-model-"));
    }
    #[test]
    fn categories_follow_the_model() {
        assert_eq!(category("demo", &[], "text-generation"), "chat");
        assert_eq!(
            category("CodeLlama-7b", &["gguf".into()], "text-generation"),
            "code"
        );
        assert_eq!(
            category("demo", &["reasoning".into()], "text-generation"),
            "reasoning"
        );
        assert_eq!(
            category("gemma3-vision", &[], "image-text-to-text"),
            "vision"
        );
    }
    #[test]
    fn picks_the_preferred_quant_and_keeps_the_projector() {
        let detail = json!({
            "sha":"067b946cf014b7c697f3654f621d577a3e3afd1c",
            "siblings":[
                {"rfilename":"demo-Q8_0.gguf","lfs":{"sha256":"b".repeat(64),"size":400}},
                {"rfilename":"demo-Q4_K_M.gguf","lfs":{"sha256":"a".repeat(64),"size":100}},
                {"rfilename":"demo-Q4_K_M-00001-of-00002.gguf","lfs":{"sha256":"c".repeat(64),"size":10}},
                {"rfilename":"mmproj-model-f16.gguf","lfs":{"sha256":"d".repeat(64),"size":50}}
            ]
        });
        let item = json!({"id":"org/demo","tags":["gguf","code"],"pipeline_tag":"text-generation","downloads":42});
        let entry = model_entry("org/demo", &detail, &item).unwrap();
        assert_eq!(entry["category"], "code");
        assert_eq!(entry["sha256"], "a".repeat(64));
        assert_eq!(entry["bytes"], 100);
        assert!(entry["file"].as_str().unwrap().starts_with("hf-org-demo-"));
        assert_eq!(
            entry["url"],
            "https://huggingface.co/org/demo/resolve/067b946cf014b7c697f3654f621d577a3e3afd1c/demo-Q4_K_M.gguf"
        );
        assert!(entry["projector"].is_null());
        let vision = json!({"id":"org/vision","tags":["gguf"],"pipeline_tag":"image-text-to-text","downloads":7});
        let entry = model_entry("org/vision", &detail, &vision).unwrap();
        assert_eq!(entry["category"], "vision");
        assert_eq!(
            entry["projector"]["file"],
            format!(
                "{}-mmproj.gguf",
                entry["id"].as_str().unwrap()
            )
        );
        assert_eq!(entry["projector"]["sha256"], "d".repeat(64));
        assert!(model_entry(
            "org/demo",
            &json!({"sha":"x","siblings":[]}),
            &item
        )
        .is_none());
    }
    #[test]
    #[ignore]
    fn live_hugging_face_search_populates_the_library() {
        let root = core::root()
            .unwrap()
            .join("tests")
            .join(format!("store-live-{}", core::id()));
        fs::create_dir_all(root.join("config")).unwrap();
        let found = search(&root, &json!({"query":"llama 3.2"}), &[], false).unwrap();
        assert!(found["added"].as_u64().unwrap() > 0, "{found}");
        let entries = crate::models::catalog(&root).unwrap();
        let store: Vec<&Value> = entries
            .iter()
            .filter(|entry| entry["origin"] == "store")
            .collect();
        assert!(!store.is_empty());
        for entry in &store {
            assert_eq!(entry["sha256"].as_str().unwrap().len(), 64);
            assert!(entry["url"]
                .as_str()
                .unwrap()
                .starts_with("https://huggingface.co/"));
            assert!(entry["file"].as_str().unwrap().ends_with(".gguf"));
            assert!(entry["bytes"].as_u64().unwrap() > 0);
            assert!(["chat", "code", "reasoning", "vision"]
                .contains(&entry["category"].as_str().unwrap()));
        }
        // The library creates a service for the store model immediately.
        let mut cfg = json!({"paths":{"models":"models","llamafile":"runtime/llamafile.exe"},"services":{}});
        crate::models::extend_config(&root, &mut cfg).unwrap();
        let id = store[0]["id"].as_str().unwrap();
        assert!(cfg["services"][id]["args"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg.as_str().unwrap_or("").ends_with(".gguf")));
        // Repeated searches do not add duplicate models.
        let again = search(&root, &json!({"query":"llama 3.2"}), &[], false).unwrap();
        assert_eq!(again["added"], 0);
        fs::remove_dir_all(&root).unwrap();
    }
    #[test]
    fn keeps_installed_and_busy_models_when_the_shelf_is_capped() {
        let dir = core::root()
            .unwrap()
            .join("tests")
            .join(format!("store-cap-{}", core::id()));
        fs::create_dir_all(&dir).unwrap();
        let mut models = vec![];
        for index in 0..MAX_STORE + 2 {
            models.push(json!({
                "id":format!("hf-model-{index}"),
                "file":format!("model-{index}.gguf"),
                "bytes":4,
                "sha256":format!("{index:064}")
            }));
        }
        // Eén geïnstalleerd model en de actieve download blijven staan.
        fs::write(dir.join("model-0.gguf"), b"GGUF").unwrap();
        core::atomic(
            &dir.join("model-0.gguf.verified.json"),
            &json!({"sha256":format!("{0:064}",0),"bytes":4}),
        )
        .unwrap();
        cap(&mut models, &dir, &["hf-model-1".into()]);
        assert_eq!(models.len(), MAX_STORE);
        assert_eq!(models[0]["id"], "hf-model-0");
        assert!(models.iter().any(|entry| entry["id"] == "hf-model-1"));
        fs::remove_dir_all(dir).unwrap();
    }
}
