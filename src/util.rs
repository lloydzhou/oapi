use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub fn oapi_home() -> PathBuf {
    if let Some(h) = env::var_os("OAPI_HOME") {
        return PathBuf::from(h);
    }
    if let Some(h) = env::var_os("BA_HOME") {
        return PathBuf::from(h).join("oapi");
    }
    env::var_os("HOME")
        .map(PathBuf::from)
        .map(|h| h.join(".local/share/oapi"))
        .unwrap_or_else(|| PathBuf::from("/tmp/oapi"))
}

pub fn apis_dir() -> PathBuf {
    oapi_home().join("apis")
}

pub fn api_path(name: &str) -> Option<PathBuf> {
    if !valid_name(name) {
        return None;
    }
    Some(apis_dir().join(format!("{}.json", name)))
}

pub fn valid_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 || name == "." || name == ".." {
        return false;
    }
    name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn ensure_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    fs::create_dir_all(path)
}

pub fn read_file<P: AsRef<Path>>(path: P) -> io::Result<String> {
    let mut f = fs::File::open(path)?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;
    Ok(s)
}

pub fn write_file<P: AsRef<Path>>(path: P, contents: &str) -> io::Result<()> {
    let tmp = path.as_ref().with_extension("tmp");
    let mut f = fs::File::create(&tmp)?;
    f.write_all(contents.as_bytes())?;
    f.flush()?;
    drop(f);
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn stdin_string() -> io::Result<String> {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s)?;
    Ok(s)
}
