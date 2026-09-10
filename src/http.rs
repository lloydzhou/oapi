use std::io::Read;
use std::time::Duration;

pub struct Response {
    pub status: u16,
    pub body: String,
}

pub fn request(method: &str, url: &str, headers: &[String], body: &str, timeout_ms: u64) -> Option<Response> {
    let req = match method.to_uppercase().as_str() {
        "GET" => ureq::get(url),
        "POST" => ureq::post(url),
        "PUT" => ureq::put(url),
        "DELETE" => ureq::delete(url),
        "PATCH" => ureq::patch(url),
        "HEAD" => ureq::head(url),
        "OPTIONS" => ureq::request("OPTIONS", url),
        _ => return None,
    };
    let mut req = req.timeout(Duration::from_millis(timeout_ms.max(1)));
    for h in headers {
        let mut parts = h.splitn(2, ':');
        let name = parts.next()?;
        let value = parts.next().unwrap_or("").trim_start();
        req = req.set(name, value);
    }
    let result = if body.is_empty() {
        req.call()
    } else {
        req.send_string(body)
    };
    let resp = match result {
        Ok(r) => r,
        Err(ureq::Error::Status(_code, r)) => r,
        Err(_) => return None,
    };
    let status = resp.status();
    let mut body = String::new();
    let _ = resp.into_reader().read_to_string(&mut body);
    Some(Response { status, body })
}
