use serde_json::{json, Value};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::util;

pub struct Registry {
    #[allow(dead_code)]
    pub name: String,
    pub source: String,
    #[allow(dead_code)]
    pub fetched_at: u64,
    pub spec: Value,
    #[allow(dead_code)]
    pub path: PathBuf,
}

pub fn load(name: &str) -> Option<Registry> {
    let path = util::api_path(name)?;
    let text = util::read_file(&path).ok()?;
    let wrapper: Value = serde_json::from_str(&text).ok()?;
    let source = wrapper.get("source")?.as_str()?.to_string();
    let fetched_at = wrapper.get("fetched_at")?.as_u64().unwrap_or(0);
    let spec = wrapper.get("spec")?.clone();
    Some(Registry {
        name: name.to_string(),
        source,
        fetched_at,
        spec,
        path,
    })
}

pub fn save(name: &str, source: &str, spec: &Value) -> io::Result<()> {
    let dir = util::apis_dir();
    util::ensure_dir(&dir)?;
    let fetched = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let wrapper = json!({
        "source": source,
        "fetched_at": fetched,
        "spec": spec,
    });
    let text = serde_json::to_string_pretty(&wrapper).unwrap();
    let path = dir.join(format!("{}.json", name));
    util::write_file(path, &text)
}

pub fn remove(name: &str) -> io::Result<()> {
    let path = util::api_path(name).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "bad name")
    })?;
    fs::remove_file(path)
}

pub fn list() -> Vec<(String, u64, String)> {
    let dir = util::apis_dir();
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            if let Some(r) = load(&name) {
                let ops = crate::spec::operations(&r.spec).len() as u64;
                out.push((name, ops, r.source));
            } else {
                out.push((name, 0, "(broken entry)".to_string()));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}
