use crate::build_info::BuildInfo;
use crate::config::Config;
use crate::event::{category_from_str, severity_from_str};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminHealth {
    pub status: &'static str,
    pub runtime: &'static str,
    pub version: &'static str,
}

impl AdminHealth {
    pub fn local() -> Self {
        Self {
            status: "ok",
            runtime: "local_cli",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\"status\":\"{}\",\"runtime\":\"{}\",\"version\":\"{}\"}}",
            self.status, self.runtime, self.version
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyExplain {
    pub version: u32,
    pub enabled_rules: usize,
    pub qmp_mappings: usize,
    pub stable_qmp_required: bool,
}

impl PolicyExplain {
    pub fn from_config(cfg: &Config) -> Self {
        Self {
            version: cfg.version,
            enabled_rules: cfg.policy.rules.iter().filter(|rule| rule.enabled).count(),
            qmp_mappings: cfg.actions.qmp.len(),
            stable_qmp_required: cfg.identity.require_stable_qmp_match,
        }
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\"version\":{},\"enabled_rules\":{},\"qmp_mappings\":{},\"stable_qmp_required\":{}}}",
            self.version, self.enabled_rules, self.qmp_mappings, self.stable_qmp_required
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyTestInput {
    pub category: String,
    pub severity: String,
    pub reason: String,
    pub vm: String,
}

pub fn validate_policy_test_input(input: &PolicyTestInput) -> Result<(), String> {
    if category_from_str(&input.category).is_none() {
        return Err(format!("unknown category '{}'", input.category));
    }
    if severity_from_str(&input.severity).is_none() {
        return Err(format!("unknown severity '{}'", input.severity));
    }
    if input.vm.trim().is_empty() {
        return Err("policy test requires a VM name".to_string());
    }
    Ok(())
}

pub fn build_info_json() -> String {
    BuildInfo::current().to_json()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_health_reports_local_cli_scope() {
        let health = AdminHealth::local();

        assert_eq!(health.runtime, "local_cli");
        assert!(health.to_json().contains("\"status\":\"ok\""));
    }

    #[test]
    fn policy_test_input_rejects_unknown_category_or_missing_vm() {
        let input = PolicyTestInput {
            category: "bogus".to_string(),
            severity: "high".to_string(),
            reason: "x".to_string(),
            vm: "vm-a".to_string(),
        };
        assert!(validate_policy_test_input(&input).is_err());

        let missing_vm = PolicyTestInput {
            category: "sensor".to_string(),
            severity: "info".to_string(),
            reason: "x".to_string(),
            vm: String::new(),
        };
        assert!(validate_policy_test_input(&missing_vm).is_err());
    }
}
