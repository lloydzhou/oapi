use serde_json::Value;

pub fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for &b in s.as_bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'%' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

pub fn append_typed_value(sb: &mut String, val: &str) {
    if let Ok(v) = serde_json::from_str::<Value>(val) {
        sb.push_str(&v.to_string());
    } else {
        sb.push_str(&serde_json::to_string(val).unwrap());
    }
}

pub fn build_body_field(key: &str, val: &str) -> String {
    let mut s = String::new();
    s.push_str(&serde_json::to_string(key).unwrap());
    s.push(':');
    append_typed_value(&mut s, val);
    s
}
