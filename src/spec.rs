use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Operation {
    pub op_id: String,
    pub kebab: String,
    pub method: String,
    pub path: String,
    pub summary: String,
    pub params: Vec<Value>,
    pub has_body: bool,
}

const HTTP_METHODS: &[&str] = &["get", "put", "post", "delete", "options", "head", "patch", "trace"];

pub fn kebab_case(s: &str) -> String {
    let mut out = String::new();
    let bytes = s.as_bytes();
    for (i, &c) in bytes.iter().enumerate() {
        let c = c as char;
        if c == '_' || c == '-' || c == ' ' {
            if !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
        } else if c.is_ascii_uppercase() {
            let prev = i.checked_sub(1).and_then(|j| bytes.get(j)).copied().map(|b| b as char);
            let next = bytes.get(i + 1).copied().map(|b| b as char);
            let need_dash = prev.map_or(false, |p| p.is_ascii_lowercase() || p.is_ascii_digit())
                || prev.map_or(false, |p| {
                    p.is_ascii_uppercase() && next.map_or(false, |n| n.is_ascii_lowercase())
                });
            if !out.is_empty() && need_dash && !out.ends_with('-') {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

pub fn resolve_pointer(root: &Value, ptr: &str) -> Option<Value> {
    if !ptr.starts_with("#/") {
        return None;
    }
    let mut cur = root;
    for seg in ptr[2..].split('/') {
        let key = seg.replace("~1", "/").replace("~0", "~");
        cur = cur.get(&key)?;
    }
    Some(cur.clone())
}

fn param_ref(spec: &Value, p: &Value) -> Value {
    if let Some(r) = p.get("$ref").and_then(|v| v.as_str()) {
        resolve_pointer(spec, r).unwrap_or(p.clone())
    } else {
        p.clone()
    }
}

fn same_param(a: &Value, b: &Value) -> bool {
    let an = a.get("name").and_then(|v| v.as_str());
    let ai = a.get("in").and_then(|v| v.as_str());
    let bn = b.get("name").and_then(|v| v.as_str());
    let bi = b.get("in").and_then(|v| v.as_str());
    an.is_some() && ai.is_some() && an == bn && ai == bi
}

fn add_param(params: &mut Vec<Value>, spec: &Value, p: &Value) {
    let p = param_ref(spec, p);
    if p.get("name").is_none() {
        return;
    }
    for existing in params.iter_mut() {
        if same_param(existing, &p) {
            *existing = p;
            return;
        }
    }
    params.push(p);
}

pub fn operations(spec: &Value) -> Vec<Operation> {
    let mut ops = Vec::new();
    let paths = match spec.get("paths").and_then(|v| v.as_object()) {
        Some(p) => p,
        None => return ops,
    };
    for (path, pathobj) in paths {
        let pathobj = match pathobj.as_object() {
            Some(o) => o,
            None => continue,
        };
        for method in HTTP_METHODS {
            let Some(op) = pathobj.get(*method).and_then(|v| v.as_object()) else {
                continue;
            };
            let Some(id) = op.get("operationId").and_then(|v| v.as_str()) else {
                continue;
            };
            let mut params = Vec::new();
            if let Some(arr) = pathobj.get("parameters").and_then(|v| v.as_array()) {
                for p in arr {
                    add_param(&mut params, spec, p);
                }
            }
            if let Some(arr) = op.get("parameters").and_then(|v| v.as_array()) {
                for p in arr {
                    add_param(&mut params, spec, p);
                }
            }
            ops.push(Operation {
                op_id: id.to_string(),
                kebab: kebab_case(id),
                method: method.to_uppercase(),
                path: path.clone(),
                summary: op.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                params,
                has_body: op.get("requestBody").is_some(),
            });
        }
    }
    ops
}

pub fn find_operation<'a>(ops: &'a [Operation], cmd: &str) -> Option<&'a Operation> {
    ops.iter()
        .find(|o| o.kebab == cmd || o.op_id == cmd)
}

pub fn base_url(source: &str, spec: &Value) -> Option<String> {    if let Some(servers) = spec.get("servers").and_then(|v| v.as_array()) {
        if let Some(s0) = servers.first() {
            if let Some(url) = s0.get("url").and_then(|v| v.as_str()) {
                if url.starts_with("http://") || url.starts_with("https://") {
                    return Some(url.to_string());
                }
                if url.starts_with('/') && (source.starts_with("http://") || source.starts_with("https://")) {
                    let origin: Vec<&str> = source.split('/').take(3).collect();
                    return Some(format!("{}{}", origin.join("/"), url));
                }
                if !url.is_empty() {
                    return Some(url.to_string());
                }
            }
        }
    }
    // Swagger 2.0
    let host = spec.get("host").and_then(|v| v.as_str()).unwrap_or("");
    if !host.is_empty() {
        let scheme = spec
            .get("schemes")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                if source.starts_with("http://") {
                    "http".to_string()
                } else {
                    "https".to_string()
                }
            });
        let base = spec.get("basePath").and_then(|v| v.as_str()).unwrap_or("");
        if base.starts_with('/') {
            return Some(format!("{}://{}{}", scheme, host, base));
        } else if !base.is_empty() {
            return Some(format!("{}://{}/{}", scheme, host, base));
        } else {
            return Some(format!("{}://{}", scheme, host));
        }
    }
    if source.starts_with("http://") || source.starts_with("https://") {
        return Some(source.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kebab() {
        assert_eq!(kebab_case("getPetById"), "get-pet-by-id");
        assert_eq!(kebab_case("getURLValue"), "get-url-value");
        assert_eq!(kebab_case("list_pets"), "list-pets");
        assert_eq!(kebab_case("simple"), "simple");
        assert_eq!(kebab_case("id_"), "id");
    }

    #[test]
    fn test_resolve_pointer() {
        let v = serde_json::json!({"components": {"schemas": {"Pet": {"type": "object"}}}});
        let r = resolve_pointer(&v, "#/components/schemas/Pet").unwrap();
        assert_eq!(r.get("type").unwrap(), "object");
        assert!(resolve_pointer(&v, "#/nope").is_none());
    }
}
