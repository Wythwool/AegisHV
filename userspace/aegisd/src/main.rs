use clap::Parser;
use std::{path::PathBuf, fs::File, io::{BufRead, BufReader}};
use prometheus::{Encoder, TextEncoder, register_counter, Counter};
use axum::{routing::get, Router};
use tokio::sync::RwLock;
use std::sync::Arc;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value_t=String::from("../events.jsonl"))]
    events: String,
    #[arg(long, default_value_t=String::from("0.0.0.0:9108"))]
    listen: String,
}

struct State { total: Counter }

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let state = Arc::new(State { total: register_counter!("aegis_events_total", "Total events").unwrap() });

    // Tail JSONL in a task and update metrics
    let p = PathBuf::from(&args.events);
    let st = state.clone();
    tokio::spawn(async move {
        let f = File::open(p).expect("open events");
        let mut rdr = BufReader::new(f);
        loop {
            let mut line = String::new();
            match rdr.read_line(&mut line) {
                Ok(0) => { tokio::time::sleep(std::time::Duration::from_millis(250)).await; }
                Ok(_) => { st.total.inc(); line.clear(); }
                Err(_) => break,
            }
        }
    });

    let app = Router::new().route("/metrics", get(|| async {
        let enc = TextEncoder::new();
        let mf = prometheus::gather();
        let mut buf = Vec::new();
        enc.encode(&mf, &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }));

    axum::Server::bind(&args.listen.parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
