use crate::event::Event;
use crate::metrics::Metrics;
use crate::tracefs::{find_trace_pipe, open_trace_pipe};
use std::io::{BufRead, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Source {
    Tracefs { root: PathBuf },
    Replay { path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlMessage {
    ReplayEof,
    CollectorError(String),
}

// Keep Event inline in the bounded ingest channel. Boxing this variant would add
// heap allocation to PMU event handoff without reducing trace line allocations.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum IngestItem {
    Line(String),
    Event(Event),
}

pub fn spawn_collector(
    source: Source,
    tx: SyncSender<IngestItem>,
    control_tx: Sender<ControlMessage>,
    stop: Arc<AtomicBool>,
    metrics: Metrics,
    queue_capacity: usize,
) -> thread::JoinHandle<Result<(), String>> {
    thread::spawn(move || match source {
        Source::Tracefs { root } => {
            collect_tracefs(&root, tx, control_tx, stop, metrics, queue_capacity)
        }
        Source::Replay { path } => {
            collect_replay(&path, tx, control_tx, stop, metrics, queue_capacity)
        }
    })
}

fn send_control(control_tx: &Sender<ControlMessage>, msg: ControlMessage) {
    // Control-plane messages must never use the lossy telemetry queue.
    let _ = control_tx.send(msg);
}

fn send_ingest(
    tx: &SyncSender<IngestItem>,
    item: IngestItem,
    metrics: &Metrics,
    queue_capacity: usize,
) {
    metrics.record_queue_send_attempt(queue_capacity);
    match tx.try_send(item) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {
            metrics.record_queue_send_rejected();
            metrics.record_queue_full(queue_capacity);
            metrics.inc_dropped("queue_full");
        }
        Err(TrySendError::Disconnected(_)) => {
            metrics.record_queue_send_rejected();
            metrics.inc_dropped("receiver_disconnected");
        }
    }
}

fn collect_tracefs(
    root: &Path,
    tx: SyncSender<IngestItem>,
    control_tx: Sender<ControlMessage>,
    stop: Arc<AtomicBool>,
    metrics: Metrics,
    queue_capacity: usize,
) -> Result<(), String> {
    let p = find_trace_pipe(root)?;
    let mut reader = open_trace_pipe(&p.trace_pipe)?;
    let mut line = String::new();
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                thread::sleep(Duration::from_millis(50));
            }
            Ok(_) => {
                let l = line.trim_end().to_string();
                metrics.inc_ingest_line();
                send_ingest(&tx, IngestItem::Line(l), &metrics, queue_capacity);
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                metrics.inc_tracefs_read_error();
                metrics.inc_collector_error();
                let msg = format!("tracefs collector read failed: {e}");
                send_control(&control_tx, ControlMessage::CollectorError(msg.clone()));
                return Err(msg);
            }
        }
    }
    Ok(())
}

pub fn collect_replay(
    path: &Path,
    tx: SyncSender<IngestItem>,
    control_tx: Sender<ControlMessage>,
    stop: Arc<AtomicBool>,
    metrics: Metrics,
    queue_capacity: usize,
) -> Result<(), String> {
    let f =
        std::fs::File::open(path).map_err(|e| format!("open replay {}: {e}", path.display()))?;
    let reader = std::io::BufReader::new(f);
    for line in reader.lines() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let l = match line {
            Ok(l) => l,
            Err(e) => {
                metrics.inc_collector_error();
                let msg = format!("replay collector read failed: {e}");
                send_control(&control_tx, ControlMessage::CollectorError(msg.clone()));
                return Err(msg);
            }
        };
        metrics.inc_ingest_line();
        send_ingest(&tx, IngestItem::Line(l), &metrics, queue_capacity);
    }
    metrics.inc_collector_eof();
    send_control(&control_tx, ControlMessage::ReplayEof);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc::{channel, sync_channel};

    fn temp_file(contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "aegishv-replay-{}-{}.log",
            std::process::id(),
            crate::util::next_sequence()
        ));
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "{}", contents).unwrap();
        path
    }

    #[test]
    fn replay_sends_eof_control_message() {
        let path = temp_file("qemu-1 [000] d..2 1.0: kvm_exit: reason EPT_VIOLATION rip 0x1 gpa 0x1000 error_code 0x4\n");
        let (tx, rx) = sync_channel(8);
        let (ctx, crx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let metrics = Metrics::new().unwrap();
        collect_replay(&path, tx, ctx, stop, metrics.clone(), 8).unwrap();
        assert!(matches!(rx.try_recv().unwrap(), IngestItem::Line(_)));
        assert_eq!(crx.try_recv().unwrap(), ControlMessage::ReplayEof);
        assert_eq!(metrics.queue_depth(), 1);
        assert_eq!(metrics.queue_capacity(), 8);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn replay_eof_survives_full_telemetry_queue() {
        let path = temp_file("line-one\nline-two\n");
        let (tx, _rx) = sync_channel(1);
        let (ctx, crx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let metrics = Metrics::new().unwrap();
        collect_replay(&path, tx, ctx, stop, metrics.clone(), 1).unwrap();
        assert_eq!(crx.try_recv().unwrap(), ControlMessage::ReplayEof);
        assert_eq!(metrics.queue_depth(), 1);
        assert_eq!(metrics.dropped_total(), 1);
        let _ = std::fs::remove_file(path);
    }
}
