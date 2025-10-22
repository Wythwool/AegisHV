use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crossbeam_channel::Sender;

use crate::{Event};
use crate::parser::{parse_trace_line, to_event};
use crate::metrics::Registry;
use crate::config::Config;
use crate::pmu::Pmu;

pub fn run_replay(file: &Path, tx: Sender<Event>, reg: &Registry, cfg: &Config, _pmu: &Pmu) -> anyhow::Result<()> {
    let f = File::open(file)?;
    let rdr = BufReader::new(f);
    let mut recent_writes: std::collections::HashMap<String, Instant> = std::collections::HashMap::new();
    for line in rdr.lines() {
        let line = line?;
        if let Some(p) = parse_trace_line(&line) {
            let mut ev = to_event(p.clone());
            if let Some(gpa) = &ev.gpa {
                if let Some(e) = &ev.ept {
                    if e.write {
                        recent_writes.insert(gpa.clone(), Instant::now());
                    } else if e.exec {
                        if let Some(t) = recent_writes.get(gpa) {
                            if t.elapsed().as_millis() <= cfg.general.wx_window_ms.into() {
                                ev.severity = "critical".into();
                                ev.message = format!("W^X violation at {}", gpa);
                                reg.inc_wx();
                            }
                        }
                    }
                }
            }
            reg.record(&ev);
            tx.send(ev).ok();
        }
    }
    Ok(())
}

pub fn run_tracefs(tracefs: &Path, tx: Sender<Event>, reg: &Registry, cfg: &Config, _pmu: &Pmu) -> anyhow::Result<()> {
    let mut candidates = vec![tracefs.join("trace_pipe")];
    candidates.push(PathBuf::from("/sys/kernel/debug/tracing/trace_pipe"));
    let file = candidates.into_iter().find(|p| p.exists()).ok_or_else(|| anyhow::anyhow!("trace_pipe not found"))?;
    let f = File::open(file)?;
    let rdr = BufReader::new(f);
    let mut recent_writes: std::collections::HashMap<String, Instant> = std::collections::HashMap::new();
    for line in rdr.lines() {
        let line = line?;
        if let Some(p) = parse_trace_line(&line) {
            let mut ev = to_event(p.clone());
            if let Some(gpa) = &ev.gpa {
                if let Some(e) = &ev.ept {
                    if e.write {
                        recent_writes.insert(gpa.clone(), Instant::now());
                    } else if e.exec {
                        if let Some(t) = recent_writes.get(gpa) {
                            if t.elapsed().as_millis() <= cfg.general.wx_window_ms.into() {
                                ev.severity = "critical".into();
                                ev.message = format!("W^X violation at {}", gpa);
                                reg.inc_wx();
                            }
                        }
                    }
                }
            }
            reg.record(&ev);
            tx.send(ev).ok();
        }
    }
    Ok(())
}

pub fn snapshot() -> anyhow::Result<serde_json::Value> {
    let obj = serde_json::json!({
        "version": 1,
        "ts": time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap(),
        "kvm": { "present": std::path::Path::new("/dev/kvm").exists() },
        "tracefs": { "tracing": std::path::Path::new("/sys/kernel/tracing").exists() }
    });
    Ok(obj)
}
