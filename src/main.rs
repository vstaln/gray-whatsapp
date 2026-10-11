//! whatsapp — WhatsApp Business Cloud API. Outbound: wa_send POSTs to
//! graph.facebook.com. Inbound: `whatsapp serve --port N` runs a local
//! webhook receiver writing ~/.gray/whatsapp/inbox.jsonl (point Meta's
//! callback at it via a tunnel — README documents the flow);
//! wa_inbox reads the ledger.
//! Config: ~/.gray/whatsapp/config.json {"phone_number_id","token","verify"}
//! or env WA_PHONE_ID / WA_TOKEN / WA_VERIFY.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use serde_json::{json, Value};

const NAME: &str = "whatsapp";
const API: &str = "https://graph.facebook.com/v21.0";

fn manifest() -> Value {
    json!({
        "name": NAME, "display_name": "WhatsApp (Cloud API)",
        "version": env!("CARGO_PKG_VERSION"), "protocol": "1.1",
        "tools": [
            {"name": "wa_send",
             "description": "Send a WhatsApp text via the Business Cloud API.",
             "parameters": {"type": "object", "properties": {
                "to": {"type": "string", "description": "E.164 digits, no +"},
                "text": {"type": "string"}}, "required": ["to","text"]}},
            {"name": "wa_inbox",
             "description": "Read inbound messages the local webhook receiver captured (wa serve).",
             "parameters": {"type": "object", "properties": {
                "limit": {"type": "integer"}, "mark_read": {"type": "boolean"}}}},
        ],
        "commands": ["/whatsapp"], "subcommands": ["whatsapp"],
    })
}

fn dir() -> std::path::PathBuf {
    let home = std::env::var_os("GRAY_HOME").map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".gray")))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    home.join("whatsapp")
}
fn config_path() -> std::path::PathBuf { dir().join("config.json") }
fn inbox_path() -> std::path::PathBuf { dir().join("inbox.jsonl") }
fn read_config() -> Value {
    std::fs::read_to_string(config_path()).ok()
        .and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(|| json!({}))
}
fn cfg(k: &str, env: &str) -> Option<String> {
    std::env::var(env).ok().filter(|s| !s.is_empty())
        .or_else(|| read_config()[k].as_str().map(str::to_string))
}

fn send(a: &Value) -> Result<String, String> {
    let pid = cfg("phone_number_id", "WA_PHONE_ID")
        .ok_or("no phone_number_id — `whatsapp setup --phone-id N --token T`")?;
    let tok = cfg("token", "WA_TOKEN").ok_or("no WA token")?;
    let url = format!("{API}/{pid}/messages");
    let body = json!({
        "messaging_product": "whatsapp",
        "to": a["to"].as_str().ok_or("to required")?,
        "type": "text",
        "text": {"body": a["text"].as_str().ok_or("text required")?},
    });
    match ureq::post(&url)
        .set("Authorization", &format!("Bearer {tok}"))
        .set("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(20))
        .send_json(body) {
        Ok(_) => Ok("sent".into()),
        Err(ureq::Error::Status(c, r)) => Err(format!("HTTP {c}: {}",
            r.into_string().unwrap_or_default().chars().take(300).collect::<String>())),
        Err(e) => Err(e.to_string()),
    }
}

fn inbox(a: &Value) -> Result<String, String> {
    let n = a["limit"].as_u64().unwrap_or(20) as usize;
    let lines: Vec<String> = std::fs::read_to_string(inbox_path()).ok()
        .map(|s| s.lines().map(str::to_string).collect()).unwrap_or_default();
    if lines.is_empty() {
        return Ok("no inbound — run `whatsapp serve --port 9090` and point Meta's webhook at it".into());
    }
    Ok(lines.iter().rev().take(n).rev().cloned().collect::<Vec<_>>().join("\n"))
}

/// Meta webhook receiver: GET verifies with hub.challenge, POST appends
/// each text message to inbox.jsonl.
fn serve(port: u16) -> Result<(), String> {
    let verify = cfg("verify", "WA_VERIFY").unwrap_or_else(|| "gray-wa".into());
    let l = TcpListener::bind(("0.0.0.0", port)).map_err(|e| e.to_string())?;
    eprintln!("whatsapp webhook on :{port} (verify token: {verify})");
    for s in l.incoming().flatten() {
        let verify = verify.clone();
        std::thread::spawn(move || {
            let mut s = s;
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            if r.read_line(&mut line).is_err() { return; }
            let mut parts = line.split_whitespace();
            let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
            // headers
            let mut clen = 0usize;
            loop {
                let mut h = String::new();
                if r.read_line(&mut h).is_err() || h.trim().is_empty() { break; }
                if h.to_lowercase().starts_with("content-length:") {
                    clen = h.split(':').nth(1).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
                }
            }
            if method == "GET" {
                // verification handshake
                let token = path.split("hub.verify_token=").nth(1)
                    .and_then(|v| v.split('&').next()).unwrap_or("");
                let challenge = path.split("hub.challenge=").nth(1)
                    .and_then(|v| v.split(' ').next()).unwrap_or("");
                if token == verify {
                    let body = challenge.as_bytes();
                    let _ = s.write_all(format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).as_bytes());
                    let _ = s.write_all(body);
                    return;
                }
                let _ = s.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n");
                return;
            }
            let mut body = vec![0u8; clen.min(256 * 1024)];
            let _ = r.read_exact(&mut body);
            if let Ok(v) = serde_json::from_slice::<Value>(&body) {
                if let Some(msgs) = v.pointer("/entry/0/changes/0/value/messages")
                    .and_then(Value::as_array) {
                    let mut f = std::fs::OpenOptions::new()
                        .create(true).append(true).open(inbox_path()).unwrap();
                    for m in msgs {
                        let _ = writeln!(f, "{} ← {}: {}",
                            m["timestamp"].as_str().unwrap_or(""),
                            m["from"].as_str().unwrap_or("?"),
                            m["text"]["body"].as_str().unwrap_or("[non-text]"));
                    }
                }
            }
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        });
    }
    Ok(())
}

fn cli(argv: &[String]) -> Result<String, String> {
    match argv.first().map(String::as_str) {
        Some("setup") => {
            let mut c = read_config();
            let mut it = argv[1..].iter();
            while let Some(f) = it.next() {
                match f.as_str() {
                    "--phone-id" => c["phone_number_id"] = json!(it.next().ok_or("--phone-id")?),
                    "--token" => c["token"] = json!(it.next().ok_or("--token")?),
                    "--verify" => c["verify"] = json!(it.next().ok_or("--verify")?),
                    o => return Err(format!("unknown flag {o}")),
                }
            }
            std::fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
            std::fs::write(config_path(), serde_json::to_string_pretty(&c).unwrap())
                .map_err(|e| e.to_string())?;
            #[cfg(unix)] {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(config_path(),
                    std::fs::Permissions::from_mode(0o600));
            }
            Ok("saved".into())
        }
        Some("serve") => {
            let port: u16 = argv.iter().position(|a| a == "--port")
                .and_then(|i| argv.get(i + 1)).and_then(|p| p.parse().ok())
                .unwrap_or(9090);
            serve(port).map(|_| "webhook server exited".into())
        }
        Some("status") | None => Ok(format!(
            "WhatsApp Cloud API\n  phone_id: {}\n  token: {}\n  inbox ledger: {}",
            cfg("phone_number_id","WA_PHONE_ID").unwrap_or("MISSING".into()),
            if cfg("token","WA_TOKEN").is_some() { "set" } else { "MISSING" },
            inbox_path().display())),
        Some("-h" | "--help" | "help") => Ok("usage: whatsapp setup | serve | status".into()),
        Some(o) => Err(format!("unknown verb {o}")),
    }
}

fn tool(name: &str, args: &Value) -> Value {
    let r = match name { "wa_send" => send(args), "wa_inbox" => inbox(args),
        t => Err(format!("unknown tool {t}")) };
    match r {
        Ok(c) => json!({"result": {"content": c}}),
        Err(e) => json!({"result": {"content": e, "is_error": true}}),
    }
}

fn sidecar() -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        let method = v["method"].as_str().unwrap_or("");
        match v.get("id").cloned() {
            None if method == "plugin/shutdown" => return Ok(()),
            None => {}
            Some(id) => {
                let reply = if method == "plugin/manifest" {
                    json!({"id": id, "result": manifest()})
                } else if method == "tool/call" {
                    let mut r = tool(v["params"]["name"].as_str().unwrap_or(""), &v["params"]["args"]);
                    r["id"] = id; r
                } else if method == "command/run" {
                    let argv: Vec<String> = v["params"]["argv"].as_array()
                        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                        .unwrap_or_default();
                    let text = cli(&argv).unwrap_or_else(|e| e);
                    json!({"id": id, "result": {"text": text}})
                } else {
                    json!({"id": id, "error": {"code": -32601,
                        "message": format!("method not found: {method}")}})
                };
                writeln!(out, "{reply}")?; out.flush()?;
            }
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<String> = match args.first().map(String::as_str) {
        Some("whatsapp") => args[1..].to_vec(), _ => args };
    match args.first().map(String::as_str) {
        Some("manifest") => println!("{}", manifest()),
        Some("sidecar") => { let _ = sidecar(); }
        _ => {
            if args.is_empty() {
                if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                    println!("whatsapp — Cloud API channel. setup → serve --port for inbound.");
                    return;
                }
                let _ = sidecar(); return;
            }
            match cli(&args) {
                Ok(t) => println!("{t}"),
                Err(e) => { eprintln!("{e}"); std::process::exit(1); }
            }
        }
    }
}
