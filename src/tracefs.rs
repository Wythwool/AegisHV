use crate::identity::VmInventorySnapshot;
use crate::trace_format::{diagnose_kvm_tracepoints, TracepointDiagnostic};
use crate::util::{json_str, now_rfc3339};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TracePipePath {
    pub trace_pipe: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub schema_version: u32,
    pub ts: String,
    pub kvm: bool,
    pub tracefs_root: String,
    pub trace_pipe: Option<String>,
    pub trace_pipe_readable: bool,
    pub tracepoints_ok: bool,
    pub tracepoints: Vec<TracepointDiagnostic>,
    pub vm_inventory: VmInventorySnapshot,
    pub mode: String,
}

impl Snapshot {
    pub fn to_json_pretty(&self) -> String {
        format!(
            "{{\n  \"schema_version\": {},\n  \"ts\": {},\n  \"kvm\": {},\n  \"tracefs_root\": {},\n  \"trace_pipe\": {},\n  \"trace_pipe_readable\": {},\n  \"tracepoints_ok\": {},\n  \"tracepoints\": {},\n  \"vm_inventory\": {},\n  \"mode\": {}\n}}",
            self.schema_version,
            json_str(&self.ts),
            self.kvm,
            json_str(&self.tracefs_root),
            self.trace_pipe.as_ref().map(|s| json_str(s)).unwrap_or_else(|| "null".to_string()),
            self.trace_pipe_readable,
            self.tracepoints_ok,
            tracepoints_json(&self.tracepoints),
            self.vm_inventory.to_json(),
            json_str(&self.mode)
        )
    }
}

fn tracepoints_json(tracepoints: &[TracepointDiagnostic]) -> String {
    let items = tracepoints
        .iter()
        .map(|diag| {
            let missing_fields = diag
                .missing_fields
                .iter()
                .map(|field| json_str(field))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"system\":{},\"name\":{},\"status\":{},\"missing_fields\":[{}],\"message\":{}}}",
                json_str(&diag.system),
                json_str(&diag.name),
                json_str(diag.status.as_str()),
                missing_fields,
                json_str(&diag.message)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{items}]")
}

pub fn find_trace_pipe(root: &Path) -> Result<TracePipePath, String> {
    let p = root.join("trace_pipe");
    if p.exists() {
        Ok(TracePipePath { trace_pipe: p })
    } else {
        Err(format!("trace_pipe not found under {}", root.display()))
    }
}

pub fn open_trace_pipe(path: &Path) -> Result<BufReader<File>, String> {
    let file = File::open(path).map_err(|e| format!("open trace_pipe {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        unsafe {
            const F_GETFL: i32 = 3;
            const F_SETFL: i32 = 4;
            const O_NONBLOCK: i32 = 0o4000;
            extern "C" {
                fn fcntl(fd: i32, cmd: i32, arg: i32) -> i32;
            }
            let fd = file.as_raw_fd();
            let flags = fcntl(fd, F_GETFL, 0);
            if flags >= 0 {
                let _ = fcntl(fd, F_SETFL, flags | O_NONBLOCK);
            }
        }
    }
    Ok(BufReader::new(file))
}

pub fn snapshot(root: &Path) -> Result<Snapshot, String> {
    snapshot_with_inventory(root, VmInventorySnapshot::empty())
}

pub fn snapshot_with_inventory(
    root: &Path,
    vm_inventory: VmInventorySnapshot,
) -> Result<Snapshot, String> {
    let trace = root.join("trace_pipe");
    let tracepoints = diagnose_kvm_tracepoints(root);
    let tracepoints_ok = tracepoints.iter().all(TracepointDiagnostic::is_ok);
    Ok(Snapshot {
        schema_version: 2,
        ts: now_rfc3339(),
        kvm: Path::new("/dev/kvm").exists(),
        tracefs_root: root.display().to_string(),
        trace_pipe: if trace.exists() {
            Some(trace.display().to_string())
        } else {
            None
        },
        trace_pipe_readable: File::open(&trace).is_ok(),
        tracepoints_ok,
        tracepoints,
        vm_inventory,
        mode: "host_side_tracefs".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_tracefs(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "aegishv-snapshot-{label}-{}-{}",
            std::process::id(),
            crate::util::next_sequence()
        ));
        std::fs::create_dir_all(path.join("events/kvm/kvm_exit")).expect("create tracefs fixture");
        std::fs::File::create(path.join("trace_pipe")).expect("create trace_pipe fixture");
        path
    }

    fn write_kvm_exit_format(root: &Path, text: &str) {
        let mut file = std::fs::File::create(root.join("events/kvm/kvm_exit/format"))
            .expect("create format fixture");
        write!(file, "{text}").expect("write format fixture");
    }

    #[test]
    fn snapshot_reports_tracepoint_metadata_health() {
        let root = temp_tracefs("ok");
        write_kvm_exit_format(
            &root,
            "name: kvm_exit\nID: 123\nformat:\n\tfield:u32 vcpu_id;\toffset:8;\tsize:4;\tsigned:0;\n\tfield:u32 exit_reason;\toffset:12;\tsize:4;\tsigned:0;\n\tfield:unsigned long guest_rip;\toffset:16;\tsize:8;\tsigned:0;\n",
        );

        let snap = snapshot(&root).expect("snapshot");
        let json = snap.to_json_pretty();

        assert!(snap.tracepoints_ok);
        assert!(json.contains("\"tracepoints_ok\": true"));
        assert!(json.contains("\"status\":\"ok\""));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn snapshot_reports_missing_tracepoint_metadata() {
        let root = temp_tracefs("missing");

        let snap = snapshot(&root).expect("snapshot");
        let json = snap.to_json_pretty();

        assert!(!snap.tracepoints_ok);
        assert!(json.contains("\"tracepoints_ok\": false"));
        assert!(json.contains("\"status\":\"missing\""));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn snapshot_json_includes_vm_inventory_contract() {
        let root = temp_tracefs("inventory");

        let snap =
            snapshot_with_inventory(&root, VmInventorySnapshot::disabled()).expect("snapshot");
        let json = snap.to_json_pretty();

        assert_eq!(snap.schema_version, 2);
        assert!(json.contains("\"vm_inventory\": {\"status\":\"disabled\""));
        assert!(json.contains("\"vm_count\":0"));
        let _ = std::fs::remove_dir_all(root);
    }
}
