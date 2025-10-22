use std::fs;
use std::path::PathBuf;

pub struct Pmu { pub enable: bool, pub sample_ms: u64 }

impl Pmu {
    pub fn new(enable: bool, sample_ms: u64) -> Self { Self { enable, sample_ms } }
    pub fn sample_vcpu_threads(&self) -> Vec<(i32, String)> {
        if !self.enable { return vec![]; }
        // Heuristic: threads with "KVM" or "CPU" in comm under qemu-system-*
        let mut out = vec![];
        if let Ok(entries) = fs::read_dir("/proc") {
            for ent in entries.flatten() {
                if let Ok(pid) = ent.file_name().to_string_lossy().parse::<i32>() {
                    let comm = fs::read_to_string(format!("/proc/{}/comm", pid)).unwrap_or_default();
                    if !comm.contains("qemu") { continue; }
                    let task_dir = PathBuf::from(format!("/proc/{}/task", pid));
                    if let Ok(tasks) = fs::read_dir(task_dir) {
                        for t in tasks.flatten() {
                            if let Ok(tid) = t.file_name().to_string_lossy().parse::<i32>() {
                                let cname = fs::read_to_string(format!("/proc/{}/task/{}/comm", pid, tid)).unwrap_or_default();
                                if cname.contains("KVM") || cname.contains("CPU") || cname.contains("vcpu") {
                                    out.push((tid, cname.trim().to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }
        out
    }
}
