use crate::collector::IngestItem;
use crate::event::{Category, Event, IdentityInfo, PmuInfo, Severity};
use crate::identity::{
    parse_vcpu_id_from_thread_name, read_proc_start_time_ticks, resolve_identity_once,
};
use crate::metrics::Metrics;
use crate::pattern::Pattern;
use crate::util::now_rfc3339;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct ThreadTarget {
    pid: i32,
    tid: i32,
    pid_start_time_ticks: Option<u64>,
    tid_start_time_ticks: Option<u64>,
    name: String,
    vm: String,
    vm_id: String,
    vcpu_id: Option<i32>,
    identity: IdentityInfo,
    last_ticks: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ThreadStat {
    start_time_ticks: u64,
    cpu_ticks: u64,
}

struct SamplerSettings {
    sample_ms: u64,
    rediscover_ms: u64,
    qemu_pid: i32,
    vm_regex: String,
    queue_capacity: usize,
}

// The sampler entry point mirrors the runtime knobs and shared handles passed
// from main; keeping the flat signature avoids an API churn-only wrapper.
#[allow(clippy::too_many_arguments)]
pub fn spawn_pmu_sampler(
    enabled: bool,
    sample_ms: u64,
    rediscover_ms: u64,
    qemu_pid: i32,
    vm_regex: String,
    tx: SyncSender<IngestItem>,
    stop: Arc<AtomicBool>,
    metrics: Metrics,
    queue_capacity: usize,
) -> Option<thread::JoinHandle<Result<(), String>>> {
    if !enabled {
        return None;
    }
    let settings = SamplerSettings {
        sample_ms,
        rediscover_ms,
        qemu_pid,
        vm_regex,
        queue_capacity,
    };
    Some(thread::spawn(move || {
        let result = run_sampler(settings, tx, stop, metrics.clone());
        if result.is_err() {
            metrics.mark_pmu_failed();
            metrics.mark_runtime_failed();
        }
        result
    }))
}

fn run_sampler(
    settings: SamplerSettings,
    tx: SyncSender<IngestItem>,
    stop: Arc<AtomicBool>,
    metrics: Metrics,
) -> Result<(), String> {
    let sample = Duration::from_millis(settings.sample_ms.max(100));
    let rediscover = rediscover_interval(settings.rediscover_ms);
    let mut last_discover = Instant::now() - rediscover;
    let mut targets: Vec<ThreadTarget> = Vec::new();
    let vm_pat = if settings.vm_regex.trim().is_empty() {
        None
    } else {
        Some(
            Pattern::compile(&settings.vm_regex)
                .map_err(|e| format!("invalid PMU vm_regex '{}': {e}", settings.vm_regex))?,
        )
    };

    while !stop.load(Ordering::Relaxed) {
        if last_discover.elapsed() >= rediscover {
            targets = list_qemu_targets(settings.qemu_pid, vm_pat.as_ref())?;
            metrics.set_pmu_targets(targets.len());
            last_discover = Instant::now();
        }
        if sleep_or_stop(sample, &stop) {
            break;
        }
        for target in &mut targets {
            let Some(thread_stat) = read_current_thread_stat_if_identity_matches(target) else {
                metrics.inc_pmu_read_failure();
                metrics.inc_dropped("pmu_pid_reuse_or_missing_start_time");
                continue;
            };
            let now_ticks = Some(thread_stat.cpu_ticks);
            let _tick_delta = match (target.last_ticks, now_ticks) {
                (Some(prev), Some(now)) => now.saturating_sub(prev),
                _ => 0,
            };
            target.last_ticks = now_ticks;
            let mut ev = Event::base(
                Category::Pmu,
                Severity::Info,
                now_rfc3339(),
                target.vm.clone(),
            );
            ev.vm_id = Some(target.vm_id.clone());
            ev.host_pid = Some(target.pid);
            ev.host_tid = Some(target.tid);
            ev.host_start_time_ticks = target.pid_start_time_ticks;
            ev.identity = Some(target.identity.clone());
            ev.vcpu_id = target.vcpu_id;
            ev.vcpu = target.vcpu_id;
            ev.reason = Some("pmu_poll".to_string());
            ev.message = Some("hardware PMU grouped sampling is not active; emitted proc-stat target heartbeat with unavailable counters as null".to_string());
            ev.pmu = Some(PmuInfo {
                pid: target.pid,
                tid: target.tid,
                thread: target.name.clone(),
                cycles_delta: None,
                instr_delta: None,
                cache_ref_delta: None,
                cache_miss_delta: None,
                branch_delta: None,
                branch_miss_delta: None,
                sample_ms: settings.sample_ms,
                source: "proc_stat_fallback".to_string(),
                grouped: false,
            });
            metrics.inc_pmu();
            metrics.record_queue_send_attempt(settings.queue_capacity);
            match tx.try_send(IngestItem::Event(ev)) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    metrics.record_queue_send_rejected();
                    metrics.record_queue_full(settings.queue_capacity);
                    metrics.inc_dropped("queue_full");
                }
                Err(TrySendError::Disconnected(_)) => {
                    metrics.record_queue_send_rejected();
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

fn sleep_or_stop(duration: Duration, stop: &AtomicBool) -> bool {
    let started = Instant::now();
    while started.elapsed() < duration {
        if stop.load(Ordering::Relaxed) {
            return true;
        }
        let remaining = duration.saturating_sub(started.elapsed());
        thread::sleep(remaining.min(Duration::from_millis(50)));
    }
    stop.load(Ordering::Relaxed)
}

fn rediscover_interval(rediscover_ms: u64) -> Duration {
    Duration::from_millis(rediscover_ms)
}

fn list_qemu_targets(
    explicit_pid: i32,
    vm_pat: Option<&Pattern>,
) -> Result<Vec<ThreadTarget>, String> {
    let mut pids = Vec::new();
    if explicit_pid > 0 {
        pids.push(explicit_pid);
    } else {
        let entries = fs::read_dir("/proc").map_err(|e| format!("read /proc: {e}"))?;
        for ent in entries {
            let ent = ent.map_err(|e| format!("read /proc entry: {e}"))?;
            let name = ent.file_name().to_string_lossy().to_string();
            let Ok(pid) = name.parse::<i32>() else {
                continue;
            };
            let comm = read_comm(&PathBuf::from(format!("/proc/{pid}/comm")));
            if let Some(c) = comm {
                let lower = c.to_ascii_lowercase();
                if lower.contains("qemu") || lower.contains("kvm") {
                    pids.push(pid);
                }
            }
        }
    }
    let mut out = Vec::new();
    for pid in pids {
        let ident = resolve_identity_once(pid, &[]);
        let Some(pid_start_time_ticks) = ident.host_start_time_ticks else {
            continue;
        };
        let proc_comm = read_comm(&PathBuf::from(format!("/proc/{pid}/comm")))
            .unwrap_or_else(|| "qemu".to_string());
        let vm_name = ident.vm_name.clone().unwrap_or(proc_comm);
        if let Some(pat) = vm_pat {
            if !pat.is_match(&vm_name) && !pat.is_match(&ident.vm_id) {
                continue;
            }
        }
        let task_dir = PathBuf::from(format!("/proc/{pid}/task"));
        let tasks = match fs::read_dir(&task_dir) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for ent in tasks {
            let ent = match ent {
                Ok(v) => v,
                Err(_) => continue,
            };
            let tid_name = ent.file_name().to_string_lossy().to_string();
            let Ok(tid) = tid_name.parse::<i32>() else {
                continue;
            };
            let tcomm = read_comm(&PathBuf::from(format!("/proc/{pid}/task/{tid}/comm")))
                .unwrap_or_else(|| "thread".to_string());
            let lower = tcomm.to_ascii_lowercase();
            if !(tcomm.contains("CPU") || tcomm.contains("KVM") || lower.contains("vcpu")) {
                continue;
            }
            let Some(thread_stat) = read_thread_stat(pid, tid) else {
                continue;
            };
            out.push(ThreadTarget {
                pid,
                tid,
                pid_start_time_ticks: Some(pid_start_time_ticks),
                tid_start_time_ticks: Some(thread_stat.start_time_ticks),
                name: tcomm.clone(),
                vm: vm_name.clone(),
                vm_id: ident.vm_id.clone(),
                vcpu_id: parse_vcpu_id_from_thread_name(&tcomm),
                identity: IdentityInfo {
                    sources: ident.identity_sources.clone(),
                    confidence: ident.identity_confidence,
                    start_time_verified: ident.start_time_verified,
                    ambiguous: ident.ambiguous,
                },
                last_ticks: Some(thread_stat.cpu_ticks),
            });
        }
    }
    Ok(out)
}

fn read_comm(p: &PathBuf) -> Option<String> {
    fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

fn read_current_thread_stat_if_identity_matches(target: &ThreadTarget) -> Option<ThreadStat> {
    let observed_pid_start = read_proc_start_time_ticks(target.pid);
    let thread_stat = read_thread_stat(target.pid, target.tid)?;
    if target_start_times_match(
        target,
        observed_pid_start,
        Some(thread_stat.start_time_ticks),
    ) {
        Some(thread_stat)
    } else {
        None
    }
}

fn target_start_times_match(
    target: &ThreadTarget,
    observed_pid_start: Option<u64>,
    observed_tid_start: Option<u64>,
) -> bool {
    matches!(
        (target.pid_start_time_ticks, observed_pid_start),
        (Some(expected), Some(observed)) if expected == observed
    ) && matches!(
        (target.tid_start_time_ticks, observed_tid_start),
        (Some(expected), Some(observed)) if expected == observed
    )
}

fn read_thread_stat(pid: i32, tid: i32) -> Option<ThreadStat> {
    let stat = fs::read_to_string(format!("/proc/{pid}/task/{tid}/stat")).ok()?;
    parse_thread_stat(&stat)
}

fn parse_thread_stat(stat: &str) -> Option<ThreadStat> {
    let rparen = stat.rfind(')')?;
    let rest = stat.get(rparen + 2..)?;
    let fields: Vec<&str> = rest.split_whitespace().collect();
    let utime = fields.get(11)?.parse::<u64>().ok()?;
    let stime = fields.get(12)?.parse::<u64>().ok()?;
    let start_time_ticks = fields.get(19)?.parse::<u64>().ok()?;
    Some(ThreadStat {
        start_time_ticks,
        cpu_ticks: utime + stime,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target_with_start_times(
        pid_start_time_ticks: Option<u64>,
        tid_start_time_ticks: Option<u64>,
    ) -> ThreadTarget {
        ThreadTarget {
            pid: 4242,
            tid: 4243,
            pid_start_time_ticks,
            tid_start_time_ticks,
            name: "CPU 0/KVM".to_string(),
            vm: "qemu-system-x86".to_string(),
            vm_id: "host-pid:4242:start:1111".to_string(),
            vcpu_id: Some(0),
            identity: IdentityInfo {
                sources: vec!["fallback_pid".to_string()],
                confidence: crate::event::IdentityConfidence::Low,
                start_time_verified: false,
                ambiguous: false,
            },
            last_ticks: Some(10),
        }
    }

    #[test]
    fn rediscover_interval_uses_configured_milliseconds() {
        assert_eq!(rediscover_interval(2500), Duration::from_millis(2500));
    }

    #[test]
    fn pmu_sleep_returns_when_stop_is_already_set() {
        let stop = AtomicBool::new(true);
        assert!(sleep_or_stop(Duration::from_secs(60), &stop));
    }

    #[test]
    fn pmu_sleep_completes_when_stop_is_clear() {
        let stop = AtomicBool::new(false);
        assert!(!sleep_or_stop(Duration::from_millis(0), &stop));
    }

    #[test]
    fn pmu_target_identity_accepts_matching_pid_and_tid_start_times() {
        let target = target_with_start_times(Some(1111), Some(2222));

        assert!(target_start_times_match(&target, Some(1111), Some(2222)));
    }

    #[test]
    fn pmu_target_identity_rejects_pid_reuse() {
        let target = target_with_start_times(Some(1111), Some(2222));

        assert!(!target_start_times_match(&target, Some(3333), Some(2222)));
    }

    #[test]
    fn pmu_target_identity_rejects_missing_start_time_metadata() {
        let target = target_with_start_times(Some(1111), Some(2222));
        let unversioned = target_with_start_times(None, Some(2222));

        assert!(!target_start_times_match(&target, Some(1111), None));
        assert!(!target_start_times_match(
            &unversioned,
            Some(1111),
            Some(2222)
        ));
    }

    #[test]
    fn parses_thread_stat_start_time_and_cpu_ticks() {
        let stat = "4243 (CPU 0/KVM) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 9999 20";

        let parsed = parse_thread_stat(stat).unwrap();

        assert_eq!(parsed.start_time_ticks, 9999);
        assert_eq!(parsed.cpu_ticks, 23);
    }
}
