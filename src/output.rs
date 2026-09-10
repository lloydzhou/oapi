use serde_json::Value;

pub fn is_sensitive_name(name: &str) -> bool {
    let norm = name.to_ascii_lowercase().replace('_', "-");
    let exact = ["authorization", "signature", "secret", "key", "apikey"];
    let suffix = ["-key", "-key-id", "-token"];
    for e in exact {
        if norm == e {
            return true;
        }
    }
    for s in suffix {
        if norm.ends_with(s) {
            return true;
        }
    }
    false
}

pub fn print_url_masked(method: &str, url: &str) {
    if let Some(q) = url.find('?') {
        print!("{} {}", method, &url[..q]);
        let rest = &url[q + 1..];
        let mut first = true;
        for pair in rest.split('&') {
            if let Some(eq) = pair.find('=') {
                let name = &pair[..eq];
                let val = &pair[eq + 1..];
                if is_sensitive_name(name) {
                    print!("{}{}=***", if first { '?' } else { '&' }, name);
                } else {
                    print!("{}{}={}", if first { '?' } else { '&' }, name, val);
                }
            } else {
                print!("{}{}", if first { '?' } else { '&' }, pair);
            }
            first = false;
        }
        println!();
    } else {
        println!("{} {}", method, url);
    }
}

pub fn print_headers_masked(headers: &[String]) {
    for h in headers {
        if let Some(colon) = h.find(':') {
            let name = &h[..colon];
            let val = h[colon + 1..].trim_start();
            if is_sensitive_name(name) {
                if let Some(sp) = val.find(' ') {
                    println!("  {}: {} ***", name, &val[..sp]);
                } else {
                    println!("  {}: ***", name);
                }
            } else {
                println!("  {}: {}", name, val);
            }
        } else {
            println!("  {}", h);
        }
    }
}

pub fn print_text(value: &Value) {
    match value {
        Value::String(s) => println!("{}", s),
        Value::Number(n) => println!("{}", n),
        Value::Bool(b) => println!("{}", b),
        Value::Null => {}
        Value::Array(arr) => {
            for v in arr {
                print_text(v);
            }
        }
        Value::Object(_) => println!("{}", value),
    }
}

pub fn print_json(value: &Value, jq_path: Option<&str>, text_out: bool) {
    if let Some(path) = jq_path {
        let matches = crate::jq::select(value, path);
        for m in matches {
            match &m {
                Value::Null => continue,
                Value::String(s) => println!("{}", s),
                _ if text_out => print_text(&m),
                _ => println!("{}", m),
            }
        }
    } else if text_out {
        print_text(value);
    } else {
        println!("{}", value);
    }
}
