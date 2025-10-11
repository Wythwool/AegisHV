use clap::Parser;
use std::{fs::File, io::Write, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};
use serde::Serialize;
use prometheus::{Encoder, TextEncoder, register_counter, register_gauge, Counter, Gauge};
use axum::{routing::get, Router};
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(target_os="linux")]
mod kvmmini;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value_t=String::from("../configs/policies.yaml"))]
    policy: String,
    #[arg(long, default_value_t=String::from("../events.jsonl"))]
    json: String,
    #[arg(long, default_value_t=String::from("0.0.0.0:9107"))]
    listen: String,
    /// Run a tiny KVM guest to produce real VMEXITs (HLT/CPUID) for the pipeline
    #[arg(long, default_value_t=false)]
    kvm_demo: bool,
}

#[derive(Serialize)]
#[serde(tag="type")]
enum Event {
    #[serde(rename="exec_trap")]
    ExecTrap { ts_ns: u128, rip: u64, cr3: u64, symbol: String },
    #[serde(rename="syscall")]
    Syscall { ts_ns: u128, nr: u32, path: String, rip: u64, callsite_hash: u64 },
    #[serde(rename="pmu")]
    PMU { ts_ns: u128, ip: u64, br_misses: u64, cycles: u64 },
    #[serde(rename="vmexit")]
    VMExit { ts_ns: u128, reason: String, rip: u64 },
    #[serde(rename="drop")]
    Drop { ts_ns: u128, lost: u64 },
}

struct Metrics {
    events: Counter,
    policy_hash: Gauge,
}

impl Metrics {
    fn new() -> Self {
        Self {
            events: register_counter!("aegis_events_total", "Total events").unwrap(),
            policy_hash: register_gauge!("aegis_policy_hash", "Policy hash").unwrap(),
        }
    }
}

fn now_ns() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let policy_bytes = std::fs::read(&args.policy)?;
    let hash = policy_bytes.iter().fold(0u64, |acc, b| acc.wrapping_mul(1099511628211).wrapping_add(*b as u64));
    let metrics = Arc::new(Metrics::new());
    metrics.policy_hash.set(hash as f64);

    let log_path = PathBuf::from(&args.json);
    let file = Arc::new(Mutex::new(File::create(&log_path)?));

    // metrics endpoint
    let app = Router::new().route("/metrics", get(|| async {
        let enc = TextEncoder::new();
        let mf = prometheus::gather();
        let mut buf = Vec::new();
        enc.encode(&mf, &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }));
    let _g = tokio::spawn(async move {
        axum::Server::bind(&args.listen.parse().unwrap())
            .serve(app.into_make_service())
            .await
            .unwrap();
    });

    if args.kvm_demo {
        #[cfg(target_os="linux")]
        {
            let exits = kvmmini::run_hlt_cpuid_demo()?;
            let mut f = file.lock().await;
            for (reason, rip) in exits {
                let e = Event::VMExit { ts_ns: now_ns(), reason, rip };
                writeln!(f, "{}", serde_json::to_string(&e)?)?;
                metrics.events.inc();
            }
        }
        #[cfg(not(target_os="linux"))]
        {
            eprintln!("kvm_demo supported only on Linux");
        }
    } else {
        // synthetic events path
        let evs = vec![
            Event::ExecTrap { ts_ns: now_ns(), rip: 0x401000, cr3: 0xdeadbeef, symbol: "guest::foo".into() },
            Event::Syscall { ts_ns: now_ns(), nr: 59, path: "/usr/bin/bash".into(), rip: 0x7fffdead, callsite_hash: 0x2a5ea4f3c01d2b77 },
            Event::PMU { ts_ns: now_ns(), ip: 0x402000, br_misses: 1234, cycles: 999999 },
        ];
        let mut f = file.lock().await;
        for e in evs {
            writeln!(f, "{}", serde_json::to_string(&e)?)?;
            metrics.events.inc();
        }
        eprintln!("devharness synthetic events written");
    }

    eprintln!("devharness ready");
    loop { tokio::time::sleep(std::time::Duration::from_secs(3600)).await; }
}
