use crate::actions::ActionDispatcher;
use crate::config::{Action, Config, Rule};
use crate::event::{category_from_str, severity_from_str, Category, Event, Severity};
use crate::metrics::Metrics;
use crate::pattern::Pattern;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct CompiledRule {
    name: String,
    id: String,
    priority: i32,
    cooldown: Duration,
    mode: RuleMode,
    actions: Vec<Action>,
    reason_re: Option<Pattern>,
    vm_re: Option<Pattern>,
    trap_type: Option<String>,
    sev_min: Option<Severity>,
    category: Option<Category>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RuleMode {
    Enforce,
    DryRun,
    Suppress,
}

pub struct PolicyEngine {
    rules: Vec<CompiledRule>,
    ignore_vm: Vec<Pattern>,
    last_fired: Mutex<HashMap<CooldownKey, Instant>>,
    actions: ActionDispatcher,
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct CooldownKey {
    rule_id: String,
    vm_scope: String,
    reason: String,
    trap_type: String,
    page: String,
    action_kinds: String,
}

impl PolicyEngine {
    pub fn new(cfg: &Config) -> Result<Self, String> {
        let mut rules = Vec::new();
        for r in &cfg.policy.rules {
            if r.enabled {
                rules.push(compile_rule(r)?);
            }
        }
        rules.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.name.cmp(&b.name))
        });
        let mut ignore_vm = Vec::new();
        for pat in &cfg.allow.ignore_vm {
            ignore_vm.push(
                Pattern::compile(pat)
                    .map_err(|e| format!("invalid ignore_vm pattern '{}': {e}", pat))?,
            );
        }
        Ok(Self {
            rules,
            ignore_vm,
            last_fired: Mutex::new(HashMap::new()),
            actions: ActionDispatcher::new(cfg)?,
        })
    }

    pub fn should_ignore_vm(&self, vm: &str) -> bool {
        self.ignore_vm.iter().any(|p| p.is_match(vm))
    }

    pub fn apply(&self, metrics: &Metrics, ev: &Event) -> Vec<Event> {
        if self.should_ignore_vm(&ev.vm) {
            return Vec::new();
        }
        for r in &self.rules {
            if !rule_matches(r, ev) {
                continue;
            }
            metrics.inc_policy_match(&r.id);
            if r.cooldown.as_millis() > 0 {
                let key = cooldown_key(r, ev);
                let now = Instant::now();
                if let Ok(mut lf) = self.last_fired.lock() {
                    if let Some(prev) = lf.get(&key) {
                        if now.saturating_duration_since(*prev) < r.cooldown {
                            metrics.inc_policy_suppressed(&r.id, "cooldown");
                            let mut ev_out = self.actions.suppress_event(
                                &r.id,
                                &ev.vm,
                                ev.vm_id.as_deref(),
                                "rule matched but cooldown is active for the same entity",
                            );
                            ev_out.identity = ev.identity.clone();
                            return vec![ev_out];
                        }
                    }
                    lf.insert(key, now);
                }
            }
            match r.mode {
                RuleMode::Suppress => {
                    metrics.inc_policy_suppressed(&r.id, "rule_mode");
                    let mut ev_out = self.actions.suppress_event(
                        &r.id,
                        &ev.vm,
                        ev.vm_id.as_deref(),
                        "rule matched in suppress mode",
                    );
                    ev_out.identity = ev.identity.clone();
                    return vec![ev_out];
                }
                RuleMode::DryRun => return self.run_actions(metrics, r, ev, false),
                RuleMode::Enforce => return self.run_actions(metrics, r, ev, true),
            }
        }
        Vec::new()
    }

    fn run_actions(
        &self,
        metrics: &Metrics,
        r: &CompiledRule,
        ev: &Event,
        execute: bool,
    ) -> Vec<Event> {
        let mut out = Vec::new();
        if r.actions.is_empty() {
            let mut ev_out = self.actions.suppress_event(
                &r.id,
                &ev.vm,
                ev.vm_id.as_deref(),
                "rule matched but has no response action",
            );
            ev_out.identity = ev.identity.clone();
            out.push(ev_out);
            return out;
        }
        for action in &r.actions {
            let mut ev_out = self.actions.run_action(
                metrics,
                Some(&r.id),
                &ev.vm,
                ev.vm_id.as_deref(),
                &action.kind,
                opt_non_empty(&action.output_path),
                opt_non_empty(&action.nic),
                ev.identity.as_ref(),
                &ev.tags,
                execute,
            );
            ev_out.identity = ev.identity.clone();
            out.push(ev_out);
        }
        out
    }
}

fn compile_rule(r: &Rule) -> Result<CompiledRule, String> {
    let reason_re = if r.match_.reason_regex.trim().is_empty() {
        None
    } else {
        Some(Pattern::compile(&r.match_.reason_regex).map_err(|e| {
            format!(
                "invalid reason_regex '{}' in rule '{}': {e}",
                r.match_.reason_regex, r.name
            )
        })?)
    };
    let vm_re = if r.match_.vm_regex.trim().is_empty() {
        None
    } else {
        Some(Pattern::compile(&r.match_.vm_regex).map_err(|e| {
            format!(
                "invalid vm_regex '{}' in rule '{}': {e}",
                r.match_.vm_regex, r.name
            )
        })?)
    };
    let sev_min = if r.match_.severity_at_least.trim().is_empty() {
        None
    } else {
        Some(
            severity_from_str(&r.match_.severity_at_least).ok_or_else(|| {
                format!(
                    "invalid severity '{}' in rule '{}'",
                    r.match_.severity_at_least, r.name
                )
            })?,
        )
    };
    let category = if r.match_.category.trim().is_empty() {
        None
    } else {
        Some(category_from_str(&r.match_.category).ok_or_else(|| {
            format!(
                "invalid category '{}' in rule '{}'",
                r.match_.category, r.name
            )
        })?)
    };
    let mode = match r.mode.as_str() {
        "enforce" => RuleMode::Enforce,
        "dry_run" => RuleMode::DryRun,
        "suppress" => RuleMode::Suppress,
        other => return Err(format!("invalid mode '{}' in rule '{}'", other, r.name)),
    };
    Ok(CompiledRule {
        name: r.name.clone(),
        id: r.id.clone(),
        priority: r.priority,
        cooldown: Duration::from_millis(r.cooldown_ms),
        mode,
        actions: r.all_actions(),
        reason_re,
        vm_re,
        trap_type: if r.match_.trap_type.trim().is_empty() {
            None
        } else {
            Some(r.match_.trap_type.clone())
        },
        sev_min,
        category,
    })
}

fn rule_matches(r: &CompiledRule, ev: &Event) -> bool {
    if let Some(cat) = r.category {
        if ev.category != cat {
            return false;
        }
    }
    if let Some(sev) = r.sev_min {
        if !ev.severity.at_least(sev) {
            return false;
        }
    }
    if let Some(tt) = &r.trap_type {
        if ev.trap_type.as_deref() != Some(tt.as_str()) {
            return false;
        }
    }
    if let Some(re) = &r.vm_re {
        let id_match = ev
            .vm_id
            .as_deref()
            .map(|id| re.is_match(id))
            .unwrap_or(false);
        if !re.is_match(&ev.vm) && !id_match {
            return false;
        }
    }
    if let Some(re) = &r.reason_re {
        let s = ev.reason.as_deref().unwrap_or("");
        if !re.is_match(s) {
            return false;
        }
    }
    true
}

fn cooldown_key(r: &CompiledRule, ev: &Event) -> CooldownKey {
    CooldownKey {
        rule_id: r.id.clone(),
        vm_scope: ev.vm_id.clone().unwrap_or_else(|| ev.vm.clone()),
        reason: ev.reason.clone().unwrap_or_default(),
        trap_type: ev.trap_type.clone().unwrap_or_default(),
        page: ev
            .addr
            .as_ref()
            .and_then(|a| a.gpa.clone())
            .unwrap_or_default(),
        action_kinds: r
            .actions
            .iter()
            .map(|a| a.kind.as_str())
            .collect::<Vec<_>>()
            .join("+"),
    }
}

fn opt_non_empty(s: &str) -> Option<&str> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        Action, Actions, Allow, DetectorSettings, General, Identity, Journald, Match,
        MetricsConfig, Pmu, Policy, Spool, Syslog, TrapSettings, WxAllow,
    };
    use crate::event::{IdentityConfidence, IdentityInfo};
    use crate::identity::{
        IDENTITY_SOURCE_FALLBACK_PID, IDENTITY_SOURCE_LIBVIRT_XML,
        IDENTITY_SOURCE_START_TIME_VERIFIED, IDENTITY_SOURCE_TRACE_COMM,
    };

    fn base_config(rule: Rule) -> Config {
        Config {
            version: 1,
            general: General::default(),
            allow: Allow::default(),
            wx_allow: WxAllow::default(),
            pmu: Pmu::default(),
            metrics: MetricsConfig::default(),
            detectors: DetectorSettings::default(),
            trap: TrapSettings::default(),
            spool: Spool::default(),
            syslog: Syslog::default(),
            journald: Journald::default(),
            identity: Identity::default(),
            actions: Actions::default(),
            policy: Policy { rules: vec![rule] },
        }
    }

    fn base_event(gpa: &str) -> Event {
        let mut ev = Event::base(
            Category::Wx,
            Severity::Critical,
            "2026-01-01T00:00:00Z".to_string(),
            "vm-a".to_string(),
        );
        ev.vm_id = Some("libvirt:vm-a".to_string());
        ev.reason = Some("W^X".to_string());
        ev.trap_type = Some("ept_violation".to_string());
        ev.addr = Some(crate::event::AddrInfo {
            rip: None,
            gva: None,
            gpa: Some(gpa.to_string()),
            qual: None,
        });
        ev
    }

    fn dry_rule(cooldown_ms: u64) -> Rule {
        Rule {
            name: "r1".to_string(),
            id: "r1".to_string(),
            match_: Match {
                category: "wx".to_string(),
                severity_at_least: "critical".to_string(),
                ..Match::default()
            },
            action: Some(Action {
                kind: "pause_vm".to_string(),
                output_path: String::new(),
                nic: String::new(),
            }),
            actions: Vec::new(),
            cooldown_ms,
            priority: 100,
            mode: "dry_run".to_string(),
            enabled: true,
        }
    }

    fn enforce_rule(cooldown_ms: u64) -> Rule {
        let mut rule = dry_rule(cooldown_ms);
        rule.mode = "enforce".to_string();
        rule
    }

    #[test]
    fn dry_run_rule_emits_audit_event() {
        let engine = PolicyEngine::new(&base_config(dry_rule(0))).unwrap();
        let metrics = Metrics::new().unwrap();
        let out = engine.apply(&metrics, &base_event("0x1000"));
        assert_eq!(out.len(), 1);
        let action = out[0].action.as_ref().unwrap();
        assert_eq!(action.status, "dry_run");
        assert_eq!(action.decision, "dry_run");
        assert_eq!(action.result, "dry_run");
        assert_eq!(action.attempt, 0);
        assert_eq!(action.max_attempts, 0);
        assert_eq!(action.retry_count, 0);
        assert_eq!(action.timeout_ms, 2000);
        assert!(!action.refused);
        assert!(!action.timed_out);
        assert_eq!(action.failure_class, None);
    }

    #[test]
    fn policy_action_copies_identity_source_metadata() {
        let engine = PolicyEngine::new(&base_config(dry_rule(0))).unwrap();
        let metrics = Metrics::new().unwrap();
        let mut input = base_event("0x1000");
        input.identity = Some(IdentityInfo {
            sources: vec![
                IDENTITY_SOURCE_LIBVIRT_XML.to_string(),
                IDENTITY_SOURCE_START_TIME_VERIFIED.to_string(),
            ],
            confidence: IdentityConfidence::High,
            start_time_verified: true,
            ambiguous: false,
        });

        let out = engine.apply(&metrics, &input);

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].identity, input.identity);
    }

    #[test]
    fn enforce_rule_refuses_low_confidence_identity_before_qmp() {
        let engine = PolicyEngine::new(&base_config(enforce_rule(0))).unwrap();
        let metrics = Metrics::new().unwrap();
        let mut input = base_event("0x1000");
        input.identity = Some(IdentityInfo {
            sources: vec![
                IDENTITY_SOURCE_TRACE_COMM.to_string(),
                IDENTITY_SOURCE_FALLBACK_PID.to_string(),
            ],
            confidence: IdentityConfidence::Low,
            start_time_verified: false,
            ambiguous: false,
        });

        let out = engine.apply(&metrics, &input);

        assert_eq!(out.len(), 1);
        let action = out[0].action.as_ref().unwrap();
        assert_eq!(action.status, "refused");
        assert_eq!(
            action.failure_class.as_deref(),
            Some("stable_identity_required")
        );
        assert!(action
            .detail
            .as_ref()
            .unwrap()
            .contains("reason=pid_only_identity"));
    }

    #[test]
    fn cooldown_is_page_scoped() {
        let engine = PolicyEngine::new(&base_config(dry_rule(60_000))).unwrap();
        let metrics = Metrics::new().unwrap();
        let out1 = engine.apply(&metrics, &base_event("0x1000"));
        let out2 = engine.apply(&metrics, &base_event("0x2000"));
        assert_eq!(out1[0].action.as_ref().unwrap().status, "dry_run");
        assert_eq!(out2[0].action.as_ref().unwrap().status, "dry_run");
    }

    #[test]
    fn cooldown_suppresses_same_entity() {
        let engine = PolicyEngine::new(&base_config(dry_rule(60_000))).unwrap();
        let metrics = Metrics::new().unwrap();
        let _ = engine.apply(&metrics, &base_event("0x1000"));
        let out2 = engine.apply(&metrics, &base_event("0x1000"));
        assert_eq!(out2[0].reason.as_deref(), Some("policy_suppressed"));
    }
}
