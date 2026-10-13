use crate::tamper::{digest_file_bounded, TamperDigest};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DumpEvidenceState {
    Accepted,
    Completed,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpEvidence {
    pub vm_id: String,
    pub path: PathBuf,
    pub state: DumpEvidenceState,
    pub qmp_accepted: bool,
    pub size_bytes: Option<u64>,
    pub digest: Option<TamperDigest>,
}

impl DumpEvidence {
    pub fn accepted(vm_id: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            vm_id: vm_id.into(),
            path: path.into(),
            state: DumpEvidenceState::Accepted,
            qmp_accepted: true,
            size_bytes: None,
            digest: None,
        }
    }

    pub fn complete(mut self, path: &Path) -> Result<Self, String> {
        if self.state != DumpEvidenceState::Accepted {
            return Err(
                "dump evidence can only complete after QMP accepted the request".to_string(),
            );
        }
        if path != self.path {
            return Err("dump evidence completion path does not match accepted path".to_string());
        }
        let metadata =
            std::fs::metadata(path).map_err(|e| format!("stat dump {}: {e}", path.display()))?;
        self.size_bytes = Some(metadata.len());
        self.digest = Some(digest_file_bounded(path)?);
        self.state = DumpEvidenceState::Completed;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn dump_evidence_keeps_qmp_acceptance_separate_from_completion() {
        let path = std::env::temp_dir().join(format!(
            "aegishv-dump-evidence-{}",
            crate::util::next_sequence()
        ));
        fs::write(&path, b"dump").unwrap();
        let accepted = DumpEvidence::accepted("libvirt:vm-a", &path);

        assert_eq!(accepted.state, DumpEvidenceState::Accepted);
        assert!(accepted.digest.is_none());

        let completed = accepted.complete(&path).unwrap();
        assert_eq!(completed.state, DumpEvidenceState::Completed);
        assert_eq!(completed.size_bytes, Some(4));
        assert!(completed.digest.unwrap().label.starts_with("fnv1a64:"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn dump_evidence_rejects_completion_for_different_path() {
        let accepted = DumpEvidence::accepted("libvirt:vm-a", "/tmp/aegishv-a.dump");

        assert!(accepted
            .complete(Path::new("/tmp/aegishv-b.dump"))
            .unwrap_err()
            .contains("does not match"));
    }
}
