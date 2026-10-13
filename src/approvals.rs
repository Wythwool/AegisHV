use crate::util::json_str;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approved,
    Denied,
}

impl ApprovalDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Denied => "denied",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAction {
    pub id: String,
    pub rule_id: String,
    pub action_kind: String,
    pub vm_id: String,
}

impl PendingAction {
    pub fn to_record(&self) -> String {
        format!(
            "id={}\nrule_id={}\naction_kind={}\nvm_id={}\n",
            self.id, self.rule_id, self.action_kind, self.vm_id
        )
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\"id\":{},\"rule_id\":{},\"action_kind\":{},\"vm_id\":{}}}",
            json_str(&self.id),
            json_str(&self.rule_id),
            json_str(&self.action_kind),
            json_str(&self.vm_id)
        )
    }
}

#[derive(Debug, Clone)]
pub struct ApprovalStore {
    dir: PathBuf,
}

impl ApprovalStore {
    pub fn new(dir: impl Into<PathBuf>) -> Result<Self, String> {
        let dir = dir.into();
        if !dir.exists() {
            return Err(format!("approval store does not exist: {}", dir.display()));
        }
        if !dir.is_dir() {
            return Err(format!(
                "approval store is not a directory: {}",
                dir.display()
            ));
        }
        Ok(Self { dir })
    }

    pub fn create_pending(&self, pending: &PendingAction) -> Result<PathBuf, String> {
        validate_id(&pending.id)?;
        let path = self.dir.join(format!("{}.pending", pending.id));
        fs::write(&path, pending.to_record())
            .map_err(|e| format!("write pending approval {}: {e}", path.display()))?;
        Ok(path)
    }

    pub fn decide(
        &self,
        id: &str,
        decision: ApprovalDecision,
        actor: &str,
    ) -> Result<PathBuf, String> {
        validate_id(id)?;
        if actor.trim().is_empty() {
            return Err("approval actor must not be empty".to_string());
        }
        let pending = self.dir.join(format!("{id}.pending"));
        if !pending.exists() {
            return Err(format!(
                "pending approval does not exist: {}",
                pending.display()
            ));
        }
        let decision_path = self.dir.join(format!("{id}.decision"));
        fs::write(
            &decision_path,
            format!("decision={}\nactor={actor}\n", decision.as_str()),
        )
        .map_err(|e| format!("write approval decision {}: {e}", decision_path.display()))?;
        Ok(decision_path)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("approval id must use letters, digits, '-' or '_'".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_store_creates_pending_records_and_decisions() {
        let dir =
            std::env::temp_dir().join(format!("aegishv-approval-{}", crate::util::next_sequence()));
        fs::create_dir(&dir).unwrap();
        let store = ApprovalStore::new(&dir).unwrap();
        let pending = PendingAction {
            id: "act-1".to_string(),
            rule_id: "rule-a".to_string(),
            action_kind: "dump_guest_memory".to_string(),
            vm_id: "libvirt:vm-a".to_string(),
        };

        let pending_path = store.create_pending(&pending).unwrap();
        let decision_path = store
            .decide("act-1", ApprovalDecision::Denied, "operator")
            .unwrap();

        assert!(pending_path.exists());
        assert!(fs::read_to_string(decision_path)
            .unwrap()
            .contains("denied"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn approval_store_rejects_path_like_ids_and_missing_store() {
        assert!(ApprovalStore::new("/definitely/missing/aegishv-approval").is_err());
        let dir = std::env::temp_dir();
        let store = ApprovalStore::new(&dir).unwrap();

        assert!(store
            .decide("../bad", ApprovalDecision::Approved, "operator")
            .unwrap_err()
            .contains("approval id"));
    }
}
