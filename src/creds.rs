//! Per-API credentials captured at `connect` time, mirroring mcpc's store:
//! values live only in an owner-only file (dir 0700, file 0600); loading
//! refuses files readable by group/other instead of sending a possibly
//! leaked credential.

use serde_json::json;
use std::fs;
use std::io;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;

use crate::util;

pub fn store(name: &str, headers: &[String]) -> io::Result<()> {
    let dir = util::creds_dir();
    util::ensure_dir(&dir)?;
    let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    let doc = json!({ "headers": headers });
    let text = serde_json::to_string_pretty(&doc).unwrap();
    let path = util::cred_path(name)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "bad name"))?;
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.flush()?;
        f.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    fs::rename(tmp, path)?;
    Ok(())
}

/// Returns the stored headers, or a problem string when the file must not be
/// used. Fail closed: a broken/refused store never degrades into a headerless
/// request.
pub fn load(name: &str) -> Result<Vec<String>, String> {
    let path = match util::cred_path(name) {
        Some(p) => p,
        None => return Ok(Vec::new()),
    };
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mode = fs::metadata(&path)
        .map(|m| m.permissions().mode())
        .unwrap_or(0o644);
    if mode & 0o077 != 0 {
        return Err(format!(
            "credential file '{}' is group/other-readable; fix with: chmod 600 {}",
            path.display(),
            path.display()
        ));
    }
    let text = util::read_file(&path).map_err(|e| e.to_string())?;
    let doc: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(doc
        .get("headers")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|h| h.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default())
}

pub fn remove(name: &str) -> io::Result<()> {
    match util::cred_path(name) {
        Some(p) if p.exists() => fs::remove_file(p),
        _ => Ok(()),
    }
}

pub fn exists(name: &str) -> bool {
    util::cred_path(name).map(|p| p.exists()).unwrap_or(false)
}
