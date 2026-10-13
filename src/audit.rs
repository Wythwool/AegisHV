use crate::util::{json_escape, json_str};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    pub actor: String,
    pub action: String,
    pub decision: String,
    pub detail: String,
}

impl AuditRecord {
    pub fn to_json_line(&self) -> String {
        format!(
            "{{\"actor\":{},\"action\":{},\"decision\":{},\"detail\":{}}}\n",
            json_str(&self.actor),
            json_str(&self.action),
            json_str(&self.decision),
            json_str(&self.detail)
        )
    }
}

#[derive(Debug, Clone)]
pub struct AppendOnlyAuditLog {
    path: PathBuf,
    max_bytes: u64,
}

impl AppendOnlyAuditLog {
    pub fn new(path: impl Into<PathBuf>, max_bytes: u64) -> Result<Self, String> {
        let path = path.into();
        if max_bytes == 0 {
            return Err("audit log max_bytes must be greater than zero".to_string());
        }
        if let Ok(meta) = fs::symlink_metadata(&path) {
            if meta.file_type().is_symlink() {
                return Err(format!(
                    "audit log path must not be a symlink: {}",
                    path.display()
                ));
            }
        }
        Ok(Self { path, max_bytes })
    }

    pub fn append(&self, record: &AuditRecord) -> Result<(), String> {
        let line = record.to_json_line();
        let current = match fs::metadata(&self.path) {
            Ok(meta) => meta.len(),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => 0,
            Err(err) => return Err(format!("stat audit log {}: {err}", self.path.display())),
        };
        if current.saturating_add(line.len() as u64) > self.max_bytes {
            return Err(format!(
                "audit log {} would exceed max_bytes={}",
                self.path.display(),
                self.max_bytes
            ));
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("open audit log {}: {e}", self.path.display()))?;
        file.write_all(line.as_bytes())
            .map_err(|e| format!("write audit log {}: {e}", self.path.display()))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn audit_detail_for_denial(reason: &str) -> String {
    format!("denied: {}", json_escape(reason))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_log_appends_json_lines_and_enforces_max_bytes() {
        let path = std::env::temp_dir().join(format!(
            "aegishv-audit-{}.jsonl",
            crate::util::next_sequence()
        ));
        let log = AppendOnlyAuditLog::new(&path, 256).unwrap();
        log.append(&AuditRecord {
            actor: "operator".to_string(),
            action: "policy_explain".to_string(),
            decision: "allowed".to_string(),
            detail: "local CLI".to_string(),
        })
        .unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"actor\":\"operator\""));

        let tiny = AppendOnlyAuditLog::new(&path, 1).unwrap();
        assert!(tiny
            .append(&AuditRecord {
                actor: "operator".to_string(),
                action: "policy_update".to_string(),
                decision: "denied".to_string(),
                detail: "too large".to_string(),
            })
            .unwrap_err()
            .contains("max_bytes"));
        let _ = fs::remove_file(path);
    }
}
