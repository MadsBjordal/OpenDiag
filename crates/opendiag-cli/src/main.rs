//! `opendiag` command-line tool.
//!
//!     opendiag devices
//!     opendiag scan <address> [--json]            e.g. sim:sim-car, serial:COM5, tcp:192.168.0.10:35000
//!     opendiag live <address> rpm,coolant_temp [--count 20]
//!     opendiag validate-definition <file.json> [--sim sim-car]
//!     opendiag serve [--port 7878] [--db file] [--ui apps/ui/dist]
//!
//! `serve` is the development bridge: it runs the real engine on 127.0.0.1 so
//! the UI can be used in a browser. It only listens on the local machine.

use opendiag_api::{Api, NoExtraLinks};
use opendiag_engine::Engine;
use opendiag_store::Store;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

fn platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "android") {
        "android"
    } else {
        "linux"
    }
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn api(db: Option<String>) -> Api {
    let store = match db {
        Some(p) => Store::open(&PathBuf::from(p)),
        None => Store::open_in_memory(),
    }
    .unwrap_or_else(|e| die(&e.to_string()));
    Api::new(Engine::new(store, platform()), Box::new(NoExtraLinks))
}

fn die(msg: &str) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(1)
}

fn call(api: &Api, v: Value) -> Value {
    let r = api.call(v, &|name, data| {
        if name == "scan_progress" {
            eprintln!(
                "  [{}/{}] {}",
                data["step"],
                data["total"],
                data["label"].as_str().unwrap_or("")
            );
        }
    });
    if r["ok"] != true {
        die(r["error"].as_str().unwrap_or("failed"));
    }
    r["data"].clone()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("devices") => {
            let a = api(None);
            for d in call(&a, json!({"cmd":"devices"})).as_array().unwrap() {
                println!(
                    "{:<40} {}  ({})",
                    d["address"].as_str().unwrap_or(""),
                    d["name"].as_str().unwrap_or(""),
                    d["detail"].as_str().unwrap_or("")
                );
            }
        }
        Some("scan") => {
            let addr = args
                .get(1)
                .unwrap_or_else(|| die("scan needs an address, e.g. sim:sim-car"));
            let a = api(arg(&args, "--db"));
            let s = call(&a, json!({"cmd":"connect","address":addr}));
            if s["vehicle_connected"] != true {
                die(&format!(
                    "adapter connected ({}), vehicle not: {}",
                    s["adapter"]["identity"], s["vehicle_error"]
                ));
            }
            let r = call(&a, json!({"cmd":"scan"}));
            if args.iter().any(|x| x == "--json") {
                println!("{}", serde_json::to_string_pretty(&r).unwrap());
                return;
            }
            if r["simulated"] == true {
                println!("*** SIMULATED VEHICLE ***");
            }
            println!(
                "Adapter:  {}",
                r["adapter"]["identity"].as_str().unwrap_or("")
            );
            println!("Protocol: {}", r["protocol"].as_str().unwrap_or("?"));
            if let Some(v) = r["vehicle"]["vin"]["vin"].as_str() {
                println!("VIN:      {v}");
            }
            println!(
                "\n{}\n{}\n",
                r["health"]["headline"].as_str().unwrap_or(""),
                r["health"]["detail"].as_str().unwrap_or("")
            );
            for d in r["dtcs"].as_array().unwrap() {
                println!(
                    "  {:<6} {:<9} {:<28} {}",
                    d["code"].as_str().unwrap_or(""),
                    d["kind"].as_str().unwrap_or(""),
                    d["ecu_label"].as_str().unwrap_or(""),
                    d["explanation"]["title"].as_str().unwrap_or("")
                );
            }
            for w in r["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .chain(r["communication_errors"].as_array().unwrap())
            {
                println!("  ! {}", w.as_str().unwrap_or(""));
            }
            call(&a, json!({"cmd":"disconnect"}));
        }
        Some("live") => {
            let addr = args.get(1).unwrap_or_else(|| die("live needs an address"));
            let keys: Vec<String> = args
                .get(2)
                .map(|k| k.split(',').map(String::from).collect())
                .unwrap_or_else(|| vec!["rpm".into(), "speed".into(), "coolant_temp".into()]);
            let count: usize = arg(&args, "--count")
                .and_then(|c| c.parse().ok())
                .unwrap_or(10);
            let a = api(None);
            call(&a, json!({"cmd":"connect","address":addr}));
            println!("time_ms,{}", keys.join(","));
            for _ in 0..count {
                let v = call(&a, json!({"cmd":"live_read","keys":keys}));
                let vals: Vec<String> = v
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| {
                        s["value"]
                            .as_f64()
                            .map(|x| format!("{x:.2}"))
                            .unwrap_or_default()
                    })
                    .collect();
                println!("{},{}", v[0]["at_ms"], vals.join(","));
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            call(&a, json!({"cmd":"disconnect"}));
        }
        Some("validate-definition") => {
            let path = args
                .get(1)
                .unwrap_or_else(|| die("validate-definition needs a file"));
            let text = std::fs::read_to_string(path).unwrap_or_else(|e| die(&e.to_string()));
            let def = match opendiag_definitions::parse(&text) {
                Ok(d) => d,
                Err(errs) => {
                    for e in errs {
                        println!("✗ {e}");
                    }
                    std::process::exit(2);
                }
            };
            println!("✓ {} ({}) is valid", def.name, def.id);
            if let Some(profile) = arg(&args, "--sim") {
                let a = api(None);
                call(
                    &a,
                    json!({"cmd":"set_setting","key":"mode","value":"expert"}),
                );
                call(
                    &a,
                    json!({"cmd":"connect","address":format!("sim:{profile}")}),
                );
                let mut failed = 0;
                for f in def.functions.iter().filter(|f| f.risk_level == 0) {
                    let r = a.call(json!({"cmd":"raw","hex":f.request}), &|_, _| {});
                    let sid = opendiag_core::parse_hex(&f.request)
                        .map(|b| b[0])
                        .unwrap_or(0);
                    let positive = r["ok"] == true
                        && r["data"]
                            .as_array()
                            .map(|m| m.iter().any(|m| m["data"][0] == sid + 0x40))
                            .unwrap_or(false);
                    println!(
                        "  {} {:<28} {}",
                        if positive { "✓" } else { "✗" },
                        f.id,
                        if positive {
                            "positive answer from the simulator"
                        } else {
                            r["error"].as_str().unwrap_or("no positive answer")
                        }
                    );
                    failed += (!positive) as i32;
                }
                println!("Changing functions (risk level ≥ 1) are not run by this check.");
                if failed > 0 {
                    std::process::exit(3);
                }
            }
        }
        Some("serve") => serve(&args),
        _ => {
            eprintln!("usage: opendiag devices | scan <address> [--json] | live <address> <keys> [--count N] | validate-definition <file> [--sim profile] | serve [--port 7878] [--db file] [--ui dir]");
            std::process::exit(64);
        }
    }
}

fn serve(args: &[String]) {
    let port = arg(args, "--port").unwrap_or_else(|| "7878".into());
    let db = arg(args, "--db").unwrap_or_else(|| "opendiag-dev.db".into());
    let ui = arg(args, "--ui").map(PathBuf::from);
    let api = Arc::new(api(Some(db.clone())));
    let server = tiny_http::Server::http(format!("127.0.0.1:{port}"))
        .unwrap_or_else(|e| die(&e.to_string()));
    eprintln!("OpenDiag development bridge on http://127.0.0.1:{port} (database {db})");
    for mut req in server.incoming_requests() {
        let api = api.clone();
        let ui = ui.clone();
        std::thread::spawn(move || {
            let origin = req
                .headers()
                .iter()
                .find(|h| h.field.equiv("Origin"))
                .map(|h| h.value.to_string());
            let mut headers =
                vec![tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap()];
            // Only pages served from this machine may call the bridge.
            if let Some(o) = origin.filter(|o| {
                o.starts_with("http://localhost:") || o.starts_with("http://127.0.0.1:")
            }) {
                headers.push(
                    tiny_http::Header::from_bytes("Access-Control-Allow-Origin", o.as_bytes())
                        .unwrap(),
                );
                headers.push(
                    tiny_http::Header::from_bytes("Access-Control-Allow-Headers", "Content-Type")
                        .unwrap(),
                );
            }
            let url = req.url().to_string();
            if req.method() == &tiny_http::Method::Options {
                let mut resp = tiny_http::Response::empty(204);
                for h in headers {
                    resp.add_header(h);
                }
                let _ = req.respond(resp);
                return;
            }
            if url == "/api" && req.method() == &tiny_http::Method::Post {
                let mut body = String::new();
                let _ = std::io::Read::read_to_string(req.as_reader(), &mut body);
                let value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                let out = api.call(value, &|_, _| {});
                let mut resp = tiny_http::Response::from_string(out.to_string());
                for h in headers {
                    resp.add_header(h);
                }
                let _ = req.respond(resp);
                return;
            }
            if let Some(dir) = ui {
                let rel = url.split('?').next().unwrap_or("/").trim_start_matches('/');
                let rel = if rel.is_empty() || rel.contains("..") {
                    "index.html"
                } else {
                    rel
                };
                let path = dir.join(rel);
                let path = if path.is_file() {
                    path
                } else {
                    dir.join("index.html")
                };
                if let Ok(bytes) = std::fs::read(&path) {
                    let ct = match path.extension().and_then(|e| e.to_str()) {
                        Some("html") => "text/html; charset=utf-8",
                        Some("js") => "text/javascript",
                        Some("css") => "text/css",
                        Some("svg") => "image/svg+xml",
                        Some("png") => "image/png",
                        _ => "application/octet-stream",
                    };
                    let resp = tiny_http::Response::from_data(bytes)
                        .with_header(tiny_http::Header::from_bytes("Content-Type", ct).unwrap());
                    let _ = req.respond(resp);
                    return;
                }
            }
            let _ =
                req.respond(tiny_http::Response::from_string("not found").with_status_code(404));
        });
    }
}
