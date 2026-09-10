mod bind;
mod http;
mod jq;
mod output;
mod registry;
mod spec;
mod util;

use serde_json::Value;
use std::env;
use std::process;

const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const MAX_HEADERS: usize = 16;

#[derive(Default)]
struct GlobalOpts {
    headers: Vec<String>,
    dry_run: bool,
    timeout_ms: u64,
}

fn usage() -> ! {
    eprintln!("usage: oapi connect NAME SPEC-URL-OR-FILE | sync NAME | ls | rm NAME | schema NAME [OP]");
    eprintln!("       oapi api NAME METHOD PATH [--params J] [--data J]");
    eprintln!("       oapi NAME OPERATION [PARAM]... [--FLAG V]... [--body X] [-F K=V]");
    eprintln!("global: --dry-run --header H:V --timeout N --insecure");
    eprintln!("call:   --body X --jq PATH -o text --header H:V");
    process::exit(1);
}

fn die(msg: &str) -> ! {
    eprintln!("oapi: {}", msg);
    process::exit(1);
}

fn err(msg: &str) {
    eprintln!("oapi: {}", msg);
}

fn parse_global_args(args: &[String]) -> (GlobalOpts, &[String]) {
    let mut opts = GlobalOpts {
        timeout_ms: DEFAULT_TIMEOUT_MS,
        ..Default::default()
    };
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--dry-run" {
            opts.dry_run = true;
            i += 1;
        } else if a == "--insecure" {
            i += 1;
        } else if a == "--header" {
            if i + 1 >= args.len() {
                die("missing value for --header");
            }
            add_header(&mut opts.headers, &args[i + 1]);
            i += 2;
        } else if let Some(v) = a.strip_prefix("--header=") {
            add_header(&mut opts.headers, v);
            i += 1;
        } else if a == "--timeout" {
            if i + 1 >= args.len() || !parse_timeout(&args[i + 1]) {
                die("invalid value for --timeout");
            }
            opts.timeout_ms = args[i + 1].parse().unwrap();
            i += 2;
        } else if let Some(v) = a.strip_prefix("--timeout=") {
            if !parse_timeout(v) {
                die("invalid value for --timeout");
            }
            opts.timeout_ms = v.parse().unwrap();
            i += 1;
        } else {
            break;
        }
    }
    (opts, &args[i..])
}

fn add_header(headers: &mut Vec<String>, h: &str) {
    if headers.len() >= MAX_HEADERS {
        die("too many headers");
    }
    if !h.contains(':') || h.starts_with(':') {
        die(&format!("invalid header '{}'", h));
    }
    headers.push(h.to_string());
}

fn parse_timeout(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) && s.len() <= 10
}

fn fetch_spec(source: &str) -> Option<Value> {
    let text = if source.starts_with("http://") || source.starts_with("https://") {
        let resp = http::request("GET", source, &["Accept: application/json".to_string()], "", DEFAULT_TIMEOUT_MS)?;
        if resp.status < 200 || resp.status >= 300 {
            err(&format!("fetch {}: HTTP {}", source, resp.status));
            return None;
        }
        resp.body
    } else {
        util::read_file(source).ok()?
    };
    serde_json::from_str(&text).ok()
}

fn cmd_connect(name: &str, source: &str, dry_run: bool) -> i32 {
    if !util::valid_name(name) {
        err(&format!("bad name '{}'", name));
        return 1;
    }
    if dry_run {
        println!("GET {}", source);
        println!("  connect {}", name);
        return 0;
    }
    let spec = match fetch_spec(source) {
        Some(s) => s,
        None => return 1,
    };
    if spec.get("paths").is_none() {
        err(&format!("{}: no paths object", source));
        return 1;
    }
    if let Err(e) = registry::save(name, source, &spec) {
        err(&format!("save failed: {}", e));
        return 1;
    }
    let ops = spec::operations(&spec);
    let kebabs: std::collections::HashSet<String> = ops.iter().map(|o| o.kebab.clone()).collect();
    if kebabs.len() != ops.len() {
        err("kebab-case operationId collision");
        return 1;
    }
    println!("connected {}: {} operations (source {})", name, ops.len(), source);
    0
}

fn cmd_sync(name: &str, opts: &GlobalOpts) -> i32 {
    let r = match registry::load(name) {
        Some(r) => r,
        None => return 1,
    };
    if r.source.is_empty() {
        die(&format!("{}: registry entry has no source", name));
    }
    cmd_connect(name, &r.source, opts.dry_run)
}

fn cmd_ls() -> i32 {
    let list = registry::list();
    if list.is_empty() {
        println!("no APIs registered (oapi connect NAME SPEC)");
        return 0;
    }
    for (name, ops, source) in list {
        println!("{:-16} {:>3} operations  {}", name, ops, source);
    }
    0
}

fn cmd_rm(name: &str) -> i32 {
    if let Err(e) = registry::remove(name) {
        err(&format!("rm failed: {}", e));
        return 1;
    }
    0
}

fn cmd_schema(r: &registry::Registry, opname: Option<&str>) -> i32 {
    let ops = spec::operations(&r.spec);
    if opname.is_none() {
        for o in &ops {
            println!(
                "{:<28} {:<6} {:<28} {}",
                o.kebab,
                o.method,
                o.path,
                o.summary
            );
        }
        return 0;
    }
    let want = opname.unwrap();
    let Some(op) = spec::find_operation(&ops, want) else {
        err(&format!("operation '{}' not found", want));
        return 1;
    };
    println!("{} {}  ({})", op.method, op.path, op.op_id);
    if !op.summary.is_empty() {
        println!("  {}", op.summary);
    }
    for p in &op.params {
        let nm = p.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        let pin = p.get("in").and_then(|v| v.as_str()).unwrap_or("?");
        let req = p.get("required").and_then(|v| v.as_bool()).unwrap_or(false);
        let typ = p
            .get("schema")
            .and_then(|v| v.get("type"))
            .or_else(|| p.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        println!(
            "  --{}  in:{}{}{}{}",
            spec::kebab_case(nm),
            pin,
            if typ.is_empty() { "" } else { " type:" },
            if typ.is_empty() { "" } else { typ },
            if req { "  (required)" } else { "" }
        );
    }
    if op.has_body {
        println!("  body: --body JSON | -F key=value");
    }
    0
}

fn cmd_api(r: &registry::Registry, method: &str, path: &str, args: &[String], opts: &GlobalOpts) -> i32 {
    let mut params = None;
    let mut data = None;
    let mut dry_run = opts.dry_run;
    let mut headers = opts.headers.clone();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--dry-run" {
            dry_run = true;
            i += 1;
        } else if a == "--insecure" {
            i += 1;
        } else if a == "--params" {
            if i + 1 >= args.len() {
                die("missing value for --params");
            }
            params = Some(args[i + 1].clone());
            i += 2;
        } else if let Some(v) = a.strip_prefix("--params=") {
            params = Some(v.to_string());
            i += 1;
        } else if a == "--data" {
            if i + 1 >= args.len() {
                die("missing value for --data");
            }
            data = Some(args[i + 1].clone());
            i += 2;
        } else if let Some(v) = a.strip_prefix("--data=") {
            data = Some(v.to_string());
            i += 1;
        } else if a == "--header" {
            if i + 1 >= args.len() {
                die("missing value for --header");
            }
            add_header(&mut headers, &args[i + 1]);
            i += 2;
        } else if let Some(v) = a.strip_prefix("--header=") {
            add_header(&mut headers, v);
            i += 1;
        } else if a == "--timeout" {
            if i + 1 >= args.len() || !parse_timeout(&args[i + 1]) {
                die("invalid value for --timeout");
            }
            i += 2;
        } else {
            die(&format!("unknown api option '{}'", a));
        }
    }
    let base = match spec::base_url(&r.source, &r.spec) {
        Some(b) => b,
        None => {
            err("spec has no usable server url");
            return 1;
        }
    };
    let mut url = trim_trailing_slash(&base);
    if path.starts_with('/') && url.ends_with('/') {
        url.push_str(&path[1..]);
    } else {
        url.push_str(path);
    }
    if let Some(q) = params {
        if q.contains('\r') || q.contains('\n') {
            die("invalid raw query");
        }
        url.push('?');
        url.push_str(&q);
    }
    if data.is_some() {
        add_header(&mut headers, "Content-Type: application/json");
    }
    if dry_run {
        output::print_url_masked(method, &url);
        output::print_headers_masked(&headers);
        if let Some(d) = data {
            println!("  body: {}", d);
        }
        return 0;
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        err(&format!("base url '{}' is not absolute", url));
        return 1;
    }
    let body = data.as_deref().unwrap_or("");
    let resp = match http::request(method, &url, &headers, body, opts.timeout_ms) {
        Some(r) => r,
        None => {
            err("request failed (transport error)");
            return 1;
        }
    };
    if resp.status < 200 || resp.status >= 300 {
        err(&format!("HTTP {}: {}", resp.status, &resp.body[..resp.body.len().min(512)]));
        return 1;
    }
    print_response_body(&resp.body, None, false);
    0
}

fn cmd_call(r: &registry::Registry, opname: &str, args: &[String], opts: &GlobalOpts) -> i32 {
    let ops = spec::operations(&r.spec);
    let Some(op) = spec::find_operation(&ops, opname) else {
        err(&format!("operation '{}' not found", opname));
        return 1;
    };

    let mut dry_run = opts.dry_run;
    let mut flag_names: Vec<String> = Vec::new();
    let mut flag_vals: Vec<Option<String>> = Vec::new();
    let mut positionals: Vec<String> = Vec::new();
    let mut body_inline: Option<String> = None;
    let mut body_fields: Vec<(String, String)> = Vec::new();
    let mut jq_path: Option<String> = None;
    let mut text_out = false;
    let mut headers = opts.headers.clone();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--dry-run" {
            dry_run = true;
            i += 1;
        } else if a == "--body" {
            if i + 1 >= args.len() {
                die("missing value for --body");
            }
            body_inline = Some(args[i + 1].clone());
            i += 2;
        } else if let Some(v) = a.strip_prefix("--body=") {
            body_inline = Some(v.to_string());
            i += 1;
        } else if a == "--jq" {
            if i + 1 >= args.len() {
                die("missing value for --jq");
            }
            jq_path = Some(args[i + 1].clone());
            i += 2;
        } else if let Some(v) = a.strip_prefix("--jq=") {
            jq_path = Some(v.to_string());
            i += 1;
        } else if a == "--header" {
            if i + 1 >= args.len() {
                die("missing value for --header");
            }
            add_header(&mut headers, &args[i + 1]);
            i += 2;
        } else if let Some(v) = a.strip_prefix("--header=") {
            add_header(&mut headers, v);
            i += 1;
        } else if a == "--timeout" {
            if i + 1 >= args.len() || !parse_timeout(&args[i + 1]) {
                die("invalid value for --timeout");
            }
            i += 2;
        } else if let Some(v) = a.strip_prefix("--timeout=") {
            if !parse_timeout(v) {
                die("invalid value for --timeout");
            }
            i += 1;
        } else if a == "--insecure" {
            i += 1;
        } else if a == "--text" {
            text_out = true;
            i += 1;
        } else if a == "-F" {
            if i + 1 >= args.len() {
                die("missing value for -F");
            }
            parse_field(&mut body_fields, &args[i + 1]);
            i += 2;
        } else if let Some(v) = a.strip_prefix("-F") {
            if v.starts_with('=') && v.len() > 1 {
                parse_field(&mut body_fields, &v[1..]);
            } else {
                die(&format!("bad -F argument '{}'", a));
            }
            i += 1;
        } else if a == "-o" {
            if i + 1 >= args.len() || args[i + 1] != "text" {
                die("-o supports 'text' only");
            }
            text_out = true;
            i += 2;
        } else if let Some(rest) = a.strip_prefix("--") {
            let (name, val) = if let Some(eq) = rest.find('=') {
                (rest[..eq].to_string(), Some(rest[eq + 1..].to_string()))
            } else {
                (rest.to_string(), None)
            };
            let val = if val.is_none() && i + 1 < args.len() && !args[i + 1].starts_with('-') {
                i += 1;
                Some(args[i].clone())
            } else {
                val
            };
            flag_names.push(name);
            flag_vals.push(val);
            i += 1;
        } else {
            positionals.push(a.clone());
            i += 1;
        }
    }

    let need_pos = op.params.iter().filter(|p| p.get("in").and_then(|v| v.as_str()) == Some("path")).count();
    if positionals.len() < need_pos {
        err(&format!("{} needs {} positional path parameter(s), got {}", opname, need_pos, positionals.len()));
        return 1;
    }
    if positionals.len() > need_pos {
        err(&format!("{} accepts {} positional path parameter(s), got {}", opname, need_pos, positionals.len()));
        return 1;
    }

    let base = match spec::base_url(&r.source, &r.spec) {
        Some(b) => b,
        None => {
            err("spec has no usable server url");
            return 1;
        }
    };
    let mut url = trim_trailing_slash(&base);
    let mut pi = 0;
    let mut path_chars = op.path.chars().peekable();
    while let Some(c) = path_chars.next() {
        if c == '{' {
            let mut name = String::new();
            while let Some(&ch) = path_chars.peek() {
                if ch == '}' {
                    path_chars.next();
                    break;
                }
                name.push(ch);
                path_chars.next();
            }
            if pi < positionals.len() {
                url.push_str(&bind::percent_encode(&positionals[pi]));
                pi += 1;
            } else {
                err(&format!("missing value for path parameter {{{}}}", name));
                return 1;
            }
        } else {
            url.push(c);
        }
    }

    let mut query = String::new();
    for (idx, name) in flag_names.iter().enumerate() {
        let mut matched = false;
        for p in &op.params {
            let pname = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let pin = p.get("in").and_then(|v| v.as_str()).unwrap_or("");
            if spec::kebab_case(pname) != *name && pname != *name {
                continue;
            }
            if pin == "path" {
                continue;
            }
            matched = true;
            if pin == "header" {
                headers.push(format!(
                    "{}: {}",
                    pname,
                    flag_vals[idx].as_deref().unwrap_or("")
                ));
            } else {
                if !query.is_empty() {
                    query.push('&');
                }
                query.push_str(&bind::percent_encode(pname));
                query.push('=');
                query.push_str(&bind::percent_encode(flag_vals[idx].as_deref().unwrap_or("true")));
            }
            break;
        }
        if !matched {
            err(&format!("unknown flag --{} (see: oapi schema)", name));
            return 1;
        }
        if flag_vals[idx].is_none() {
            let is_bool = op.params.iter().any(|p| {
                let pname = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let pin = p.get("in").and_then(|v| v.as_str()).unwrap_or("");
                let typ = p
                    .get("schema")
                    .and_then(|v| v.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                (spec::kebab_case(pname) == *name || pname == *name) && pin == "query" && typ == "boolean"
            });
            if !is_bool {
                err(&format!("missing value for --{}", name));
                return 1;
            }
        }
    }
    if !query.is_empty() {
        url.push('?');
        url.push_str(&query);
    }

    let mut body = String::new();
    if let Some(b) = &body_inline {
        if !body_fields.is_empty() {
            die("--body and -F are mutually exclusive");
        }
        if b.starts_with('@') {
            body = util::read_file(&b[1..]).unwrap_or_else(|_| die(&format!("cannot read {}", &b[1..])));
        } else if b == "-" {
            body = util::stdin_string().unwrap_or_default();
        } else {
            body = b.clone();
        }
    } else if !body_fields.is_empty() {
        body.push('{');
        for (i, (k, v)) in body_fields.iter().enumerate() {
            if i > 0 {
                body.push(',');
            }
            body.push_str(&bind::build_body_field(k, v));
        }
        body.push('}');
    }
    if !body.is_empty() {
        add_header(&mut headers, "Content-Type: application/json");
    }

    if dry_run {
        output::print_url_masked(&op.method, &url);
        output::print_headers_masked(&headers);
        if !body.is_empty() {
            println!("  body: {}", body);
        }
        return 0;
    }

    if !url.starts_with("http://") && !url.starts_with("https://") {
        err(&format!("base url '{}' is not absolute", url));
        return 1;
    }
    let resp = match http::request(&op.method, &url, &headers, &body, opts.timeout_ms) {
        Some(r) => r,
        None => {
            err("request failed (transport error)");
            return 1;
        }
    };
    if resp.status < 200 || resp.status >= 300 {
        err(&format!("HTTP {}: {}", resp.status, &resp.body[..resp.body.len().min(512)]));
        return 1;
    }
    print_response_body(&resp.body, jq_path.as_deref(), text_out);
    0
}

fn parse_field(out: &mut Vec<(String, String)>, s: &str) {
    let Some(eq) = s.find('=') else {
        die(&format!("bad -F argument '{}' (want K=V)", s));
    };
    if eq == 0 {
        die(&format!("bad -F argument '{}' (want K=V)", s));
    }
    out.push((s[..eq].to_string(), s[eq + 1..].to_string()));
}

fn print_response_body(body: &str, jq_path: Option<&str>, text_out: bool) {
    if let Ok(v) = serde_json::from_str::<Value>(body) {
        output::print_json(&v, jq_path, text_out);
    } else {
        print!("{}", body);
        if !body.ends_with('\n') {
            println!();
        }
    }
}

fn trim_trailing_slash(s: &str) -> String {
    s.trim_end_matches('/').to_string()
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let (opts, rest) = parse_global_args(&args);
    if rest.is_empty() {
        usage();
    }
    if rest[0].starts_with('-') {
        die(&format!("invalid global option '{}'", rest[0]));
    }

    let rc = match rest[0].as_str() {
        "connect" => {
            if rest.len() != 3 {
                usage();
            }
            cmd_connect(&rest[1], &rest[2], opts.dry_run)
        }
        "sync" => {
            if rest.len() != 2 {
                usage();
            }
            cmd_sync(&rest[1], &opts)
        }
        "ls" => cmd_ls(),
        "rm" => {
            if rest.len() != 2 {
                usage();
            }
            cmd_rm(&rest[1])
        }
        "schema" => {
            if rest.len() < 2 || rest.len() > 3 {
                usage();
            }
            let r = registry::load(&rest[1]).unwrap_or_else(|| die(&format!("unknown API '{}'", rest[1])));
            cmd_schema(&r, rest.get(2).map(|s| s.as_str()))
        }
        "api" => {
            if rest.len() < 4 {
                usage();
            }
            let r = registry::load(&rest[1]).unwrap_or_else(|| die(&format!("unknown API '{}'", rest[1])));
            cmd_api(&r, &rest[2], &rest[3], &rest[4..], &opts)
        }
        name => {
            if rest.len() < 2 {
                usage();
            }
            let r = registry::load(name).unwrap_or_else(|| die(&format!("unknown API '{}'", name)));
            cmd_call(&r, &rest[1], &rest[2..], &opts)
        }
    };
    process::exit(rc);
}
