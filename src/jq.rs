use serde_json::Value;

pub fn select(value: &Value, path: &str) -> Vec<Value> {
    let mut cur = vec![value.clone()];
    let mut seg = String::new();
    let mut in_bracket = false;
    for ch in path.chars() {
        if ch == '[' {
            if !seg.is_empty() {
                cur = step_key(cur, &seg);
                seg.clear();
            }
            in_bracket = true;
        } else if ch == ']' {
            if in_bracket {
                cur = step_index(cur, &seg);
                seg.clear();
            }
            in_bracket = false;
        } else if ch == '.' && !in_bracket {
            if !seg.is_empty() {
                cur = step_key(cur, &seg);
                seg.clear();
            }
        } else {
            seg.push(ch);
        }
    }
    if !seg.is_empty() {
        cur = step_key(cur, &seg);
    }
    cur
}

fn step_key(values: Vec<Value>, key: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for v in values {
        if let Some(obj) = v.as_object() {
            if let Some(val) = obj.get(key) {
                out.push(val.clone());
            }
        }
    }
    out
}

fn step_index(values: Vec<Value>, idx: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for v in values {
        if let Some(arr) = v.as_array() {
            if idx == "*" || idx.is_empty() {
                out.extend(arr.iter().cloned());
            } else if let Ok(i) = idx.parse::<usize>() {
                if let Some(val) = arr.get(i) {
                    out.push(val.clone());
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let v = serde_json::json!({"a":{"b":[{"c":1},{"c":2}]}});
        assert_eq!(select(&v, "a.b[0].c"), vec![serde_json::json!(1)]);
        assert_eq!(select(&v, "a.b[].c"), vec![serde_json::json!(1), serde_json::json!(2)]);
    }
}
