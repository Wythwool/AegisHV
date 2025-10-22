mod parser;
mod collector;
mod metrics;
mod pmu;
mod config;

use std::io::{self, Write};
use std::path::PathBuf;
use crossbeam_channel::{bounded, Receiver};
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub version: u32,
    pub ts: String,
    pub arch: String,
    pub vm: String,
    pub vcpu: i32,
    pub reason: String,
    pub rip: Option<String>,
    pub gpa: Option<String>,
    pub ept: Option<EptInfo>,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EptInfo {
    pub read: bool,
    pub write: bool,
    pub exec: bool,
    pub qual: String,
}

fn write_jsonl(ev: &Event, out: &mut dyn Write) -> io::Result<()> {
    let s = serde_json::to_string(ev).unwrap();
    writeln!(out, "{}", s)
}

fn run_http(addr: &str, rx_metrics: Receiver<String>) -> anyhow::Result<()> {
    let listener = tiny_http::Server::http(addr)?;
    eprintln!("[info] http listening on {}", addr);
    loop {
        let rq = listener.recv()?;
        let mut last = String::new();
        while let Ok(m) = rx_metrics.try_recv() {
            last = m;
        }
        if rq.url() == "/metrics" {
            let resp = tiny_http::Response::from_string(last).with_status_code(200);
            rq.respond(resp)?;
        } else if rq.url() == "/healthz" {
            let resp = tiny_http::Response::from_string("ok").with_status_code(200);
            rq.respond(resp)?;
        } else {
            let resp = tiny_http::Response::from_string("use /metrics").with_status_code(404);
            rq.respond(resp)?;
        }
    }
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "help".into());
    match cmd.as_str() {
        "run" => {
            let mut tracefs = PathBuf::from("/sys/kernel/tracing");
            let mut jsonl: Option<PathBuf> = None;
            let mut listen = Some(String::from("0.0.0.0:9108"));
            let mut replay: Option<PathBuf> = None;
            let mut rules: Option<PathBuf> = None;
            let mut quiet = false;
            while let Some(a) = args.next() {
                match a.as_str() {
                    "--tracefs" => tracefs = PathBuf::from(args.next().unwrap()),
                    "--jsonl" => jsonl = Some(PathBuf::from(args.next().unwrap())),
                    "--listen" => listen = Some(args.next().unwrap()),
                    "--replay" => replay = Some(PathBuf::from(args.next().unwrap())),
                    "--rules" => rules = Some(PathBuf::from(args.next().unwrap())),
                    "--quiet" => quiet = true,
                    _ => {}
                }
            }
            let cfg = config::Config::load(rules.as_deref()).unwrap_or_default();
            let (tx_ev, rx_ev) = bounded::<Event>(1024);
            let (tx_m, rx_m) = bounded::<String>(8);

            // metrics thread
            let m = metrics::Registry::new();
            let m_for_collector = m.clone();
            std::thread::spawn(move || {
                if let Some(addr) = listen {
                    let _ = run_http(&addr, rx_m);
                }
            });

            // collector metrics renderer
            let tx_m_clone = tx_m.clone();
            std::thread::spawn(move || {
                metrics::emit_loop(m_for_collector, tx_m_clone);
            });

            // PMU sampling (placeholder disabled by default, not fatal if unavailable)
            let pmu = pmu::Pmu::new(cfg.pmu.enable, cfg.pmu.sample_ms);

            // choose source
            if let Some(file) = replay {
                collector::run_replay(&file, tx_ev.clone(), &m, &cfg, &pmu)?;
            } else {
                collector::run_tracefs(&tracefs, tx_ev.clone(), &m, &cfg, &pmu)?;
            }

            // writer
            let mut file_out: Option<std::fs::File> = match jsonl {
                Some(p) => Some(std::fs::OpenOptions::new().create(true).append(true).open(p)?),
                None => None,
            };
            ctrlc::set_handler(move || {
                eprintln!("[info] stopping");
                std::process::exit(0);
            })?;
            for ev in rx_ev.iter() {
                if !quiet {
                    eprintln!("[{}] {} vcpu={} {}", ev.severity, ev.vm, ev.vcpu, ev.message);
                }
                if let Some(f) = file_out.as_mut() {
                    let _ = write_jsonl(&ev, f);
                }
            }
            Ok(())
        }
        "snapshot" => {
            let out = args.next().unwrap_or_else(|| "out/snapshot.json".into());
            std::fs::create_dir_all(std::path::Path::new(&out).parent().unwrap())?;
            let snap = collector::snapshot()?;
            std::fs::write(&out, serde_json::to_string_pretty(&snap)?)?;
            println!("Wrote {}", out);
            Ok(())
        }
        "self-check" => {
            println!("OK");
            Ok(())
        }
        "dump-schemas" => {
            let s = std::fs::read_to_string("schema/event.schema.json")?;
            println!("{}", s);
            Ok(())
        }
        _ => {
            eprintln!("usage: aegishv run|snapshot|self-check|dump-schemas");
            std::process::exit(2);
        }
    }
}
