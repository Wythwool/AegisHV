use crate::config::{Config, QmpMapping};
use crate::event::{ActionInfo, Category, Event, IdentityConfidence, IdentityInfo, Severity};
use crate::identity::{
    IDENTITY_SOURCE_AMBIGUOUS, IDENTITY_SOURCE_FALLBACK_PID, IDENTITY_SOURCE_LIBVIRT_LIFECYCLE,
    IDENTITY_SOURCE_LIBVIRT_XML, IDENTITY_SOURCE_PROC_CGROUP, IDENTITY_SOURCE_PROC_CMDLINE,
    IDENTITY_SOURCE_QMP_SOCKET_HINT, IDENTITY_SOURCE_START_TIME_VERIFIED,
};
use crate::metrics::{IdentityQmpRefusalReason, Metrics};
use crate::pattern::Pattern;
use crate::util::{json_escape, now_rfc3339};
use std::fmt;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;

#[derive(Clone)]
pub struct ActionDispatcher {
    qmp: Vec<CompiledQmpMapping>,
    require_stable_qmp_match: bool,
    min_action_confidence: IdentityConfidence,
    timeout: Duration,
    retries: u32,
    dump_root: PathBuf,
}

#[derive(Clone)]
struct CompiledQmpMapping {
    vm_id: Option<Pattern>,
    vm: Option<Pattern>,
    socket: String,
}

struct ActionOutcome {
    ok: bool,
    status: String,
    detail: Option<String>,
    target_vm_id: Option<String>,
}

struct ActionExecution {
    outcome: Result<ActionOutcome, ActionFailure>,
    attempts: u32,
    max_attempts: u32,
    retry_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionSafetyRefusalReason {
    MissingIdentity,
    LowConfidence,
    AmbiguousIdentity,
    UnverifiedIdentity,
    PidOnlyIdentity,
    StaleIdentity,
    ConflictingIdentity,
}

impl ActionSafetyRefusalReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::MissingIdentity => "missing_identity",
            Self::LowConfidence => "low_confidence",
            Self::AmbiguousIdentity => "ambiguous_identity",
            Self::UnverifiedIdentity => "unverified_identity",
            Self::PidOnlyIdentity => "pid_only_identity",
            Self::StaleIdentity => "stale_identity",
            Self::ConflictingIdentity => "conflicting_identity",
        }
    }

    fn detail(self, kind: &str, required: IdentityConfidence) -> String {
        let cause = match self {
            Self::MissingIdentity => "event has no identity metadata",
            Self::LowConfidence => "identity confidence is below the configured action threshold",
            Self::AmbiguousIdentity => "identity is ambiguous",
            Self::UnverifiedIdentity => "identity lacks PID/TID start-time verification",
            Self::PidOnlyIdentity => "identity is based on PID-only fallback metadata",
            Self::StaleIdentity => "identity has a stale-cache conflict",
            Self::ConflictingIdentity => "identity sources conflict",
        };
        format!(
            "identity_safety: reason={} action={} required_confidence={}; refused QMP action because {}",
            self.as_str(),
            kind,
            required.as_str(),
            cause
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActionFailure {
    class: ActionFailureClass,
    detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionFailureClass {
    QmpError,
    Timeout,
    Refusal,
    UnsupportedAction,
    UnsafeInput,
    MissingArgument,
}

impl ActionFailure {
    fn error(detail: impl Into<String>) -> Self {
        let detail = detail.into();
        let class = if is_timeout_detail(&detail) {
            ActionFailureClass::Timeout
        } else {
            ActionFailureClass::QmpError
        };
        Self { class, detail }
    }

    fn refused(detail: impl Into<String>) -> Self {
        Self {
            class: ActionFailureClass::Refusal,
            detail: detail.into(),
        }
    }

    fn unsupported(detail: impl Into<String>) -> Self {
        Self {
            class: ActionFailureClass::UnsupportedAction,
            detail: detail.into(),
        }
    }

    fn unsafe_input(detail: impl Into<String>) -> Self {
        Self {
            class: ActionFailureClass::UnsafeInput,
            detail: detail.into(),
        }
    }

    fn missing_argument(detail: impl Into<String>) -> Self {
        Self {
            class: ActionFailureClass::MissingArgument,
            detail: detail.into(),
        }
    }

    fn status(&self) -> &'static str {
        match self {
            Self {
                class: ActionFailureClass::Timeout,
                ..
            } => "timeout",
            Self {
                class: ActionFailureClass::Refusal,
                ..
            } => "refused",
            Self {
                class: ActionFailureClass::UnsupportedAction,
                ..
            } => "unsupported",
            Self {
                class: ActionFailureClass::UnsafeInput | ActionFailureClass::MissingArgument,
                ..
            } => "refused",
            Self { .. } => "error",
        }
    }

    fn failure_class(&self) -> &'static str {
        match self {
            Self {
                class: ActionFailureClass::QmpError,
                ..
            } => "qmp_error",
            Self {
                class: ActionFailureClass::Timeout,
                ..
            } => "timeout",
            Self {
                class: ActionFailureClass::Refusal,
                ..
            } => "stable_identity_required",
            Self {
                class: ActionFailureClass::UnsupportedAction,
                ..
            } => "unsupported_action",
            Self {
                class: ActionFailureClass::UnsafeInput,
                ..
            } => "unsafe_input",
            Self {
                class: ActionFailureClass::MissingArgument,
                ..
            } => "missing_argument",
        }
    }

    fn is_retryable(&self) -> bool {
        matches!(
            self.class,
            ActionFailureClass::QmpError | ActionFailureClass::Timeout
        )
    }

    fn is_refused(&self) -> bool {
        matches!(
            self.class,
            ActionFailureClass::Refusal
                | ActionFailureClass::UnsafeInput
                | ActionFailureClass::MissingArgument
        )
    }

    fn is_timeout(&self) -> bool {
        self.class == ActionFailureClass::Timeout
    }

    fn identity_qmp_refusal_reason(&self, vm_id: Option<&str>) -> Option<IdentityQmpRefusalReason> {
        if self.class != ActionFailureClass::Refusal {
            return None;
        }
        if let Some(reason) = identity_safety_metric_reason(&self.detail) {
            return Some(reason);
        }
        if vm_id.is_some_and(|id| id.starts_with("ambiguous:")) {
            return Some(IdentityQmpRefusalReason::AmbiguousIdentity);
        }
        if self.detail.contains("matched multiple actions.qmp sockets")
            || self.detail.contains("different vm_id")
        {
            return Some(IdentityQmpRefusalReason::ConflictingStableMapping);
        }
        Some(IdentityQmpRefusalReason::StableIdentityRequired)
    }

    fn into_detail(self) -> String {
        self.detail
    }
}

impl From<String> for ActionFailure {
    fn from(detail: String) -> Self {
        ActionFailure::error(detail)
    }
}

fn is_timeout_detail(detail: &str) -> bool {
    let lower = detail.to_ascii_lowercase();
    lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("would block")
        || lower.contains("did not respond")
        || lower.contains("temporarily unavailable")
}

fn identity_safety_metric_reason(detail: &str) -> Option<IdentityQmpRefusalReason> {
    if !detail.starts_with("identity_safety:") {
        return None;
    }
    if detail.contains("reason=missing_identity") {
        Some(IdentityQmpRefusalReason::MissingIdentity)
    } else if detail.contains("reason=low_confidence") {
        Some(IdentityQmpRefusalReason::LowConfidence)
    } else if detail.contains("reason=ambiguous_identity") {
        Some(IdentityQmpRefusalReason::AmbiguousIdentity)
    } else if detail.contains("reason=unverified_identity") {
        Some(IdentityQmpRefusalReason::UnverifiedIdentity)
    } else if detail.contains("reason=pid_only_identity") {
        Some(IdentityQmpRefusalReason::PidOnlyIdentity)
    } else if detail.contains("reason=stale_identity") {
        Some(IdentityQmpRefusalReason::StaleIdentity)
    } else if detail.contains("reason=conflicting_identity") {
        Some(IdentityQmpRefusalReason::ConflictingIdentity)
    } else {
        Some(IdentityQmpRefusalReason::StableIdentityRequired)
    }
}

fn qmp_backed_action(kind: &str) -> bool {
    matches!(
        kind,
        "pause_vm" | "resume_vm" | "dump_guest_memory" | "quarantine_nic"
    )
}

fn has_identity_source(identity: &IdentityInfo, source: &str) -> bool {
    identity.sources.iter().any(|candidate| candidate == source)
}

fn is_pid_only_identity(identity: &IdentityInfo) -> bool {
    has_identity_source(identity, IDENTITY_SOURCE_FALLBACK_PID)
        && ![
            IDENTITY_SOURCE_PROC_CMDLINE,
            IDENTITY_SOURCE_PROC_CGROUP,
            IDENTITY_SOURCE_LIBVIRT_XML,
            IDENTITY_SOURCE_LIBVIRT_LIFECYCLE,
            IDENTITY_SOURCE_QMP_SOCKET_HINT,
            IDENTITY_SOURCE_START_TIME_VERIFIED,
        ]
        .iter()
        .any(|source| has_identity_source(identity, source))
}

fn has_identity_conflict_tag(tags: &[String], reason: &str) -> bool {
    let expected = format!("identity_conflict:{reason}");
    tags.iter().any(|tag| tag == &expected)
}

fn has_any_identity_conflict_tag(tags: &[String]) -> bool {
    tags.iter()
        .any(|tag| tag == "identity:conflict" || tag.starts_with("identity_conflict:"))
}

impl ActionOutcome {
    fn completed(target_vm_id: Option<String>) -> Self {
        Self {
            ok: true,
            status: "completed".to_string(),
            detail: None,
            target_vm_id,
        }
    }

    fn accepted(detail: Option<String>, target_vm_id: Option<String>) -> Self {
        Self {
            ok: true,
            status: "accepted".to_string(),
            detail,
            target_vm_id,
        }
    }
}

impl ActionDispatcher {
    pub fn new(cfg: &Config) -> Result<Self, String> {
        if cfg.identity.min_action_confidence == IdentityConfidence::Low {
            return Err(
                "invalid identity.min_action_confidence: low confidence cannot authorize QMP actions"
                    .to_string(),
            );
        }
        let mut qmp = Vec::new();
        for m in &cfg.actions.qmp {
            qmp.push(compile_qmp_mapping(m)?);
        }
        Ok(Self {
            qmp,
            require_stable_qmp_match: cfg.identity.require_stable_qmp_match,
            min_action_confidence: cfg.identity.min_action_confidence,
            timeout: Duration::from_millis(cfg.actions.timeout_ms),
            retries: cfg.actions.retries,
            dump_root: PathBuf::from(&cfg.actions.dump_root),
        })
    }

    fn qmp_socket_for<'a>(
        &'a self,
        vm_id: Option<&str>,
        vm_name: &str,
    ) -> Result<&'a str, ActionFailure> {
        if let Some(id) = vm_id {
            if id.starts_with("ambiguous:") {
                return Err(ActionFailure::refused(format!(
                    "refused QMP action for vm '{}': VM identity is ambiguous ({})",
                    vm_name, id
                )));
            }
        }
        if let Some(id) = vm_id {
            if let Some(socket) = self.unique_vm_id_socket(id, vm_name)? {
                return Ok(socket);
            }
        }
        if self.require_stable_qmp_match {
            let detail = if let Some(id) = vm_id {
                format!(
                    "identity.require_stable_qmp_match=true refused QMP VM-name fallback: no actions.qmp vm_id pattern matched vm_id '{}' for vm '{}'",
                    id, vm_name
                )
            } else {
                format!(
                    "identity.require_stable_qmp_match=true refused QMP VM-name fallback: event for vm '{}' has no stable vm_id",
                    vm_name
                )
            };
            return Err(ActionFailure::refused(detail));
        }

        // VM-name fallback is an explicit degraded mode. UUID or other stable
        // vm_id mappings are resolved first and conflicting stable mappings
        // refuse before name fallback can select a socket.
        if let Some(socket) = self.unique_vm_name_socket(vm_id, vm_name)? {
            return Ok(socket);
        }
        Err(ActionFailure::error(format!(
            "no QMP socket mapped for vm_id '{}' vm '{}'",
            vm_id.unwrap_or("none"),
            vm_name
        )))
    }

    fn identity_safety_refusal(
        &self,
        vm_id: Option<&str>,
        kind: &str,
        identity: Option<&IdentityInfo>,
        identity_tags: &[String],
    ) -> Option<ActionSafetyRefusalReason> {
        if !qmp_backed_action(kind) {
            return None;
        }
        if vm_id.is_some_and(|id| id.starts_with("ambiguous:")) {
            return None;
        }
        if has_identity_conflict_tag(identity_tags, "stale_cache") {
            return Some(ActionSafetyRefusalReason::StaleIdentity);
        }
        if has_any_identity_conflict_tag(identity_tags) {
            return Some(ActionSafetyRefusalReason::ConflictingIdentity);
        }
        let Some(identity) = identity else {
            return Some(ActionSafetyRefusalReason::MissingIdentity);
        };
        if identity.ambiguous || has_identity_source(identity, IDENTITY_SOURCE_AMBIGUOUS) {
            return Some(ActionSafetyRefusalReason::AmbiguousIdentity);
        }
        if is_pid_only_identity(identity) {
            return Some(ActionSafetyRefusalReason::PidOnlyIdentity);
        }
        if !identity.confidence.meets(self.min_action_confidence) {
            return Some(ActionSafetyRefusalReason::LowConfidence);
        }
        if !identity.start_time_verified
            || !has_identity_source(identity, IDENTITY_SOURCE_START_TIME_VERIFIED)
        {
            return Some(ActionSafetyRefusalReason::UnverifiedIdentity);
        }
        None
    }

    fn unique_vm_id_socket<'a>(
        &'a self,
        vm_id: &str,
        vm_name: &str,
    ) -> Result<Option<&'a str>, ActionFailure> {
        let mut selected: Option<&str> = None;
        for m in &self.qmp {
            let Some(pattern) = &m.vm_id else {
                continue;
            };
            if !pattern.is_match(vm_id) {
                continue;
            }

            let socket = m.socket.as_str();
            if let Some(existing) = selected {
                if existing != socket {
                    return Err(ActionFailure::refused(format!(
                        "refused QMP action for vm '{}': vm_id '{}' matched multiple actions.qmp sockets; configure one UUID-authoritative mapping",
                        vm_name, vm_id
                    )));
                }
            } else {
                selected = Some(socket);
            }
        }
        Ok(selected)
    }

    fn unique_vm_name_socket<'a>(
        &'a self,
        vm_id: Option<&str>,
        vm_name: &str,
    ) -> Result<Option<&'a str>, ActionFailure> {
        let mut selected: Option<&str> = None;
        for m in &self.qmp {
            if let Some(p) = &m.vm {
                if p.is_match(vm_name) {
                    if let (Some(id), Some(stable_pattern)) = (vm_id, &m.vm_id) {
                        if !stable_pattern.is_match(id) {
                            return Err(ActionFailure::refused(format!(
                                "refused QMP action for vm '{}': VM-name fallback matched an actions.qmp entry for a different vm_id; configure a matching UUID-authoritative mapping",
                                vm_name
                            )));
                        }
                    }

                    let socket = m.socket.as_str();
                    if let Some(existing) = selected {
                        if existing != socket {
                            return Err(ActionFailure::refused(format!(
                                "refused QMP action for vm '{}': VM-name fallback matched multiple actions.qmp sockets; add a stable vm_id mapping or narrow the name patterns",
                                vm_name
                            )));
                        }
                    } else {
                        selected = Some(socket);
                    }
                }
            }
        }
        Ok(selected)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_action(
        &self,
        metrics: &Metrics,
        rule_id: Option<&str>,
        vm: &str,
        vm_id: Option<&str>,
        kind: &str,
        output_path: Option<&str>,
        nic: Option<&str>,
        identity: Option<&IdentityInfo>,
        identity_tags: &[String],
        execute: bool,
    ) -> Event {
        let t0 = Instant::now();
        let mut ev = Event::base(
            Category::Policy,
            Severity::Info,
            now_rfc3339(),
            vm.to_string(),
        );
        ev.vm_id = vm_id.map(|s| s.to_string());
        ev.reason = Some("policy_action".to_string());
        ev.rule_id = rule_id.map(|s| s.to_string());
        ev.decision = Some(if execute {
            "executed".to_string()
        } else {
            "dry_run".to_string()
        });

        let execution = if execute {
            if let Some(reason) = self.identity_safety_refusal(vm_id, kind, identity, identity_tags)
            {
                ActionExecution {
                    outcome: Err(ActionFailure::refused(
                        reason.detail(kind, self.min_action_confidence),
                    )),
                    attempts: 1,
                    max_attempts: 1,
                    retry_count: 0,
                }
            } else {
                self.run_with_retries(vm, vm_id, kind, output_path, nic)
            }
        } else {
            ActionExecution {
                outcome: Ok(ActionOutcome {
                    ok: true,
                    status: "dry_run".to_string(),
                    detail: Some("action not executed".to_string()),
                    target_vm_id: vm_id.map(|s| s.to_string()),
                }),
                attempts: 0,
                max_attempts: 0,
                retry_count: 0,
            }
        };

        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
        metrics.observe_action_latency_ms(elapsed_ms);
        if execute {
            metrics.observe_qmp_latency_ms(elapsed_ms);
        }

        let attempts = execution.attempts;
        let max_attempts = execution.max_attempts;
        let retry_count = execution.retry_count;
        let (ok, status, detail, target_vm_id, failure_class, timed_out, refused) =
            match execution.outcome {
                Ok(v) => (v.ok, v.status, v.detail, v.target_vm_id, None, false, false),
                Err(e) => {
                    metrics.inc_qmp_failure(kind);
                    if let Some(reason) = e.identity_qmp_refusal_reason(vm_id) {
                        metrics.inc_identity_qmp_safety_refusal(reason);
                    }
                    let status = e.status().to_string();
                    if status == "refused" {
                        ev.decision = Some("refused".to_string());
                    }
                    let failure_class = Some(e.failure_class().to_string());
                    let timed_out = e.is_timeout();
                    let refused = e.is_refused();
                    (
                        false,
                        status,
                        Some(e.into_detail()),
                        vm_id.map(|s| s.to_string()),
                        failure_class,
                        timed_out,
                        refused,
                    )
                }
            };
        metrics.inc_policy_action(kind, ok);
        ev.action_id = Some(format!("act-{}", ev.event_id));
        ev.action_status = Some(status.clone());
        ev.action = Some(ActionInfo {
            rule: rule_id.map(|s| s.to_string()),
            kind: kind.to_string(),
            ok,
            status: status.clone(),
            decision: ev.decision.clone().unwrap_or_else(|| "unknown".to_string()),
            result: status,
            detail,
            latency_ms: Some(elapsed_ms.round() as u64),
            target_vm_id,
            attempt: attempts,
            max_attempts,
            retry_count,
            timeout_ms: self.timeout.as_millis().min(u128::from(u64::MAX)) as u64,
            timed_out,
            refused,
            failure_class,
        });
        if !ok {
            ev.severity = Severity::High;
        }
        ev
    }

    pub fn suppress_event(
        &self,
        rule_id: &str,
        vm: &str,
        vm_id: Option<&str>,
        detail: &str,
    ) -> Event {
        let mut ev = Event::base(
            Category::Policy,
            Severity::Info,
            now_rfc3339(),
            vm.to_string(),
        );
        ev.vm_id = vm_id.map(|s| s.to_string());
        ev.reason = Some("policy_suppressed".to_string());
        ev.rule_id = Some(rule_id.to_string());
        ev.decision = Some("suppressed".to_string());
        ev.message = Some(detail.to_string());
        ev
    }

    fn run_with_retries(
        &self,
        vm: &str,
        vm_id: Option<&str>,
        kind: &str,
        output_path: Option<&str>,
        nic: Option<&str>,
    ) -> ActionExecution {
        let mut last_error: Option<ActionFailure> = None;
        let max_attempts = self.retries.saturating_add(1);
        let mut attempts = 0u32;
        for attempt in 0..=self.retries {
            attempts = attempts.saturating_add(1);
            let result = match kind {
                "pause_vm" => self.pause_vm(vm, vm_id),
                "resume_vm" => self.resume_vm(vm, vm_id),
                "dump_guest_memory" => {
                    self.dump_guest_memory(vm, vm_id, Path::new(output_path.unwrap_or("")))
                }
                "quarantine_nic" => self.quarantine_nic(vm, vm_id, nic.unwrap_or("")),
                "manual_approval" => Ok(ActionOutcome::accepted(
                    Some("manual approval required; no QMP command executed".to_string()),
                    vm_id.map(|s| s.to_string()),
                )),
                "noop" => Ok(ActionOutcome::completed(vm_id.map(|s| s.to_string()))),
                _ => Err(ActionFailure::unsupported(format!(
                    "unknown action kind: {kind}"
                ))),
            };
            match result {
                Ok(outcome) => {
                    return ActionExecution {
                        outcome: Ok(outcome),
                        attempts,
                        max_attempts,
                        retry_count: attempts.saturating_sub(1),
                    }
                }
                Err(err) => {
                    let retryable = err.is_retryable();
                    last_error = Some(err);
                    if retryable && attempt < self.retries {
                        thread::sleep(Duration::from_millis(50 * (attempt as u64 + 1)));
                    } else {
                        break;
                    }
                }
            }
        }
        ActionExecution {
            outcome: Err(last_error.unwrap_or_else(|| {
                ActionFailure::error("action failed without error detail".to_string())
            })),
            attempts,
            max_attempts,
            retry_count: attempts.saturating_sub(1),
        }
    }

    fn pause_vm(&self, vm: &str, vm_id: Option<&str>) -> Result<ActionOutcome, ActionFailure> {
        let sock = self.qmp_socket_for(vm_id, vm)?;
        let mut q = QmpClient::connect(sock, self.timeout)?;
        q.exec("stop", None)?;
        Ok(ActionOutcome::completed(vm_id.map(|s| s.to_string())))
    }

    fn resume_vm(&self, vm: &str, vm_id: Option<&str>) -> Result<ActionOutcome, ActionFailure> {
        let sock = self.qmp_socket_for(vm_id, vm)?;
        let mut q = QmpClient::connect(sock, self.timeout)?;
        q.exec("cont", None)?;
        Ok(ActionOutcome::completed(vm_id.map(|s| s.to_string())))
    }

    fn dump_guest_memory(
        &self,
        vm: &str,
        vm_id: Option<&str>,
        out: &Path,
    ) -> Result<ActionOutcome, ActionFailure> {
        let sock = self.qmp_socket_for(vm_id, vm)?;
        validate_dump_output_path(out, &self.dump_root)
            .map_err(|e| ActionFailure::unsafe_input(e.to_string()))?;
        let protocol = format!("file:{}", out.display());
        let mut q = QmpClient::connect(sock, self.timeout)?;
        let args = format!(
            "{{\"paging\":true,\"protocol\":\"{}\",\"detach\":true}}",
            json_escape(&protocol)
        );
        q.exec("dump-guest-memory", Some(&args))?;
        Ok(ActionOutcome::accepted(Some("QMP accepted dump-guest-memory; completion must be verified by QMP job/event tracking before using the file as evidence".to_string()), vm_id.map(|s| s.to_string())))
    }

    fn quarantine_nic(
        &self,
        vm: &str,
        vm_id: Option<&str>,
        nic: &str,
    ) -> Result<ActionOutcome, ActionFailure> {
        let sock = self.qmp_socket_for(vm_id, vm)?;
        if nic.trim().is_empty() {
            return Err(ActionFailure::missing_argument(
                "quarantine_nic requires action.nic (QMP netdev name)".to_string(),
            ));
        }
        let mut q = QmpClient::connect(sock, self.timeout)?;
        let args = format!("{{\"name\":\"{}\",\"up\":false}}", json_escape(nic));
        q.exec("set_link", Some(&args))?;
        Ok(ActionOutcome::completed(vm_id.map(|s| s.to_string())))
    }
}

fn compile_qmp_mapping(m: &QmpMapping) -> Result<CompiledQmpMapping, String> {
    Ok(CompiledQmpMapping {
        vm_id: if m.vm_id.trim().is_empty() {
            None
        } else {
            Some(
                Pattern::compile(&m.vm_id)
                    .map_err(|e| format!("invalid qmp vm_id pattern '{}': {e}", m.vm_id))?,
            )
        },
        vm: if m.vm.trim().is_empty() {
            None
        } else {
            Some(
                Pattern::compile(&m.vm)
                    .map_err(|e| format!("invalid qmp vm pattern '{}': {e}", m.vm))?,
            )
        },
        socket: m.socket.clone(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DumpPathError {
    MissingOutputPath,
    OutputNotAbsolute,
    OutputContainsParent,
    MissingParent(PathBuf),
    ParentNotDirectory(PathBuf),
    OutputFinalSymlink(PathBuf),
    OutputExists(PathBuf),
    DumpRootNotAbsolute(PathBuf),
    DumpRootContainsParent(PathBuf),
    DumpRootMissing(PathBuf),
    DumpRootNotDirectory(PathBuf),
    DumpRootSymlink(PathBuf),
    SymlinkAncestor {
        role: &'static str,
        path: PathBuf,
    },
    OutsideDumpRoot {
        parent: PathBuf,
        root: PathBuf,
    },
    UnsafeMode {
        role: &'static str,
        path: PathBuf,
    },
    Stat {
        op: &'static str,
        path: PathBuf,
        source: String,
    },
}

impl fmt::Display for DumpPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOutputPath => write!(f, "dump_guest_memory requires action.output_path"),
            Self::OutputNotAbsolute => {
                write!(f, "dump_guest_memory output_path must be absolute")
            }
            Self::OutputContainsParent => write!(
                f,
                "dump_guest_memory output_path must not contain .. components"
            ),
            Self::MissingParent(path) => {
                write!(f, "dump output parent does not exist: {}", path.display())
            }
            Self::ParentNotDirectory(path) => {
                write!(
                    f,
                    "dump output parent must be a directory: {}",
                    path.display()
                )
            }
            Self::OutputFinalSymlink(path) => {
                write!(
                    f,
                    "dump output path must not be a symlink: {}",
                    path.display()
                )
            }
            Self::OutputExists(path) => {
                write!(f, "dump output path already exists: {}", path.display())
            }
            Self::DumpRootNotAbsolute(path) => {
                write!(f, "dump_root must be absolute: {}", path.display())
            }
            Self::DumpRootContainsParent(path) => {
                write!(
                    f,
                    "dump_root must not contain .. components: {}",
                    path.display()
                )
            }
            Self::DumpRootMissing(path) => {
                write!(f, "dump_root does not exist: {}", path.display())
            }
            Self::DumpRootNotDirectory(path) => {
                write!(f, "dump_root must be a directory: {}", path.display())
            }
            Self::DumpRootSymlink(path) => {
                write!(f, "dump_root must not be a symlink: {}", path.display())
            }
            Self::SymlinkAncestor { role, path } => {
                write!(
                    f,
                    "{role} must not have a symlink ancestor: {}",
                    path.display()
                )
            }
            Self::OutsideDumpRoot { parent, root } => {
                write!(
                    f,
                    "dump output parent {} must stay under dump_root {}",
                    parent.display(),
                    root.display()
                )
            }
            Self::UnsafeMode { role, path } => {
                write!(
                    f,
                    "{role} must not be group/world-writable: {}",
                    path.display()
                )
            }
            Self::Stat { op, path, source } => {
                write!(f, "{op} {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for DumpPathError {}

pub fn validate_dump_output_path(out: &Path, dump_root: &Path) -> Result<(), DumpPathError> {
    if out.as_os_str().is_empty() {
        return Err(DumpPathError::MissingOutputPath);
    }
    if !out.is_absolute() {
        return Err(DumpPathError::OutputNotAbsolute);
    }
    if out.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(DumpPathError::OutputContainsParent);
    }
    if !dump_root.is_absolute() {
        return Err(DumpPathError::DumpRootNotAbsolute(dump_root.to_path_buf()));
    }
    if dump_root
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(DumpPathError::DumpRootContainsParent(
            dump_root.to_path_buf(),
        ));
    }

    let root_meta = match std::fs::symlink_metadata(dump_root) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Err(DumpPathError::DumpRootMissing(dump_root.to_path_buf()))
        }
        Err(e) => {
            return Err(DumpPathError::Stat {
                op: "stat dump_root",
                path: dump_root.to_path_buf(),
                source: e.to_string(),
            })
        }
    };
    if root_meta.file_type().is_symlink() {
        return Err(DumpPathError::DumpRootSymlink(dump_root.to_path_buf()));
    }
    if !root_meta.is_dir() {
        return Err(DumpPathError::DumpRootNotDirectory(dump_root.to_path_buf()));
    }
    reject_symlink_ancestors(dump_root, "dump_root")?;
    reject_group_world_writable(dump_root, "dump_root", &root_meta)?;

    let parent = out
        .parent()
        .ok_or_else(|| DumpPathError::MissingParent(out.to_path_buf()))?;
    match std::fs::symlink_metadata(out) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(DumpPathError::OutputFinalSymlink(out.to_path_buf()));
            }
            return Err(DumpPathError::OutputExists(out.to_path_buf()));
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => {
            return Err(DumpPathError::Stat {
                op: "stat dump output path",
                path: out.to_path_buf(),
                source: e.to_string(),
            })
        }
    }

    let parent_meta = match std::fs::symlink_metadata(parent) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Err(DumpPathError::MissingParent(parent.to_path_buf()))
        }
        Err(e) => {
            return Err(DumpPathError::Stat {
                op: "stat dump output parent",
                path: parent.to_path_buf(),
                source: e.to_string(),
            })
        }
    };
    if parent_meta.file_type().is_symlink() {
        return Err(DumpPathError::SymlinkAncestor {
            role: "dump output path",
            path: parent.to_path_buf(),
        });
    }
    if !parent_meta.is_dir() {
        return Err(DumpPathError::ParentNotDirectory(parent.to_path_buf()));
    }
    reject_symlink_ancestors(parent, "dump output path")?;
    reject_group_world_writable(parent, "dump output parent", &parent_meta)?;

    let canon_parent = std::fs::canonicalize(parent).map_err(|e| DumpPathError::Stat {
        op: "canonicalize dump output parent",
        path: parent.to_path_buf(),
        source: e.to_string(),
    })?;
    let canon_root = std::fs::canonicalize(dump_root).map_err(|e| DumpPathError::Stat {
        op: "canonicalize dump_root",
        path: dump_root.to_path_buf(),
        source: e.to_string(),
    })?;
    if !canon_parent.starts_with(&canon_root) {
        return Err(DumpPathError::OutsideDumpRoot {
            parent: canon_parent,
            root: canon_root,
        });
    }
    Ok(())
}

fn reject_symlink_ancestors(path: &Path, role: &'static str) -> Result<(), DumpPathError> {
    let mut ancestors: Vec<&Path> = path
        .ancestors()
        .filter(|p| !p.as_os_str().is_empty())
        .collect();
    ancestors.reverse();
    for ancestor in ancestors {
        let meta = std::fs::symlink_metadata(ancestor).map_err(|e| DumpPathError::Stat {
            op: "stat path ancestor",
            path: ancestor.to_path_buf(),
            source: e.to_string(),
        })?;
        if meta.file_type().is_symlink() {
            return Err(DumpPathError::SymlinkAncestor {
                role,
                path: ancestor.to_path_buf(),
            });
        }
    }
    Ok(())
}

#[cfg(unix)]
fn reject_group_world_writable(
    path: &Path,
    role: &'static str,
    meta: &std::fs::Metadata,
) -> Result<(), DumpPathError> {
    use std::os::unix::fs::MetadataExt;
    if meta.mode() & 0o022 != 0 {
        Err(DumpPathError::UnsafeMode {
            role,
            path: path.to_path_buf(),
        })
    } else {
        Ok(())
    }
}

#[cfg(not(unix))]
fn reject_group_world_writable(
    _path: &Path,
    _role: &'static str,
    _meta: &std::fs::Metadata,
) -> Result<(), DumpPathError> {
    Ok(())
}

#[cfg(unix)]
struct QmpClient {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
    timeout: Duration,
}

#[cfg(unix)]
impl QmpClient {
    fn connect(sock: &str, timeout: Duration) -> Result<Self, String> {
        let stream = UnixStream::connect(sock).map_err(|e| format!("connect QMP {sock}: {e}"))?;
        let _ = stream.set_read_timeout(Some(timeout));
        let _ = stream.set_write_timeout(Some(timeout));
        let reader = BufReader::new(
            stream
                .try_clone()
                .map_err(|e| format!("clone QMP stream: {e}"))?,
        );
        let mut c = Self {
            stream,
            reader,
            timeout,
        };
        let _ = c
            .read_one_json()
            .map_err(|e| format!("read QMP greeting: {e}"))?;
        c.exec("qmp_capabilities", None)
            .map_err(|e| format!("qmp_capabilities: {e}"))?;
        Ok(c)
    }

    fn exec(&mut self, cmd: &str, args: Option<&str>) -> Result<(), String> {
        let obj = if let Some(args) = args {
            format!(
                "{{\"execute\":\"{}\",\"arguments\":{}}}",
                json_escape(cmd),
                args
            )
        } else {
            format!("{{\"execute\":\"{}\"}}", json_escape(cmd))
        };
        self.stream
            .write_all(obj.as_bytes())
            .map_err(|e| format!("write QMP command: {e}"))?;
        self.stream
            .write_all(b"\n")
            .map_err(|e| format!("write QMP newline: {e}"))?;
        self.stream
            .flush()
            .map_err(|e| format!("flush QMP command: {e}"))?;
        let started = Instant::now();
        while started.elapsed() <= self.timeout {
            let msg = self.read_one_json()?;
            if msg.contains("\"return\"") {
                return Ok(());
            }
            if msg.contains("\"error\"") {
                return Err(format!("QMP error for {cmd}: {msg}"));
            }
        }
        Err(format!("QMP did not respond for {cmd}"))
    }

    fn read_one_json(&mut self) -> Result<String, String> {
        let mut s = String::new();
        let n = self
            .reader
            .read_line(&mut s)
            .map_err(|e| format!("read QMP line: {e}"))?;
        if n == 0 {
            return Err("QMP EOF".to_string());
        }
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err("empty QMP line".to_string());
        }
        if !(trimmed.starts_with('{') && trimmed.ends_with('}')) {
            return Err(format!("invalid QMP json frame: {trimmed}"));
        }
        Ok(trimmed.to_string())
    }
}

#[cfg(not(unix))]
struct QmpClient;

#[cfg(not(unix))]
impl QmpClient {
    fn connect(_sock: &str, _timeout: Duration) -> Result<Self, String> {
        Err("QMP unix sockets are only supported on Unix".to_string())
    }
    fn exec(&mut self, _cmd: &str, _args: Option<&str>) -> Result<(), String> {
        Err("QMP unix sockets are only supported on Unix".to_string())
    }
}

#[cfg(test)]
mod stable_match_tests {
    use super::*;
    use crate::config::QmpMapping;
    use crate::identity::IDENTITY_SOURCE_TRACE_COMM;
    use crate::metrics::Metrics;

    fn config_with_qmp(require_stable: bool, vm_id: &str, vm: &str) -> Config {
        let mut cfg = Config::default();
        cfg.identity.require_stable_qmp_match = require_stable;
        cfg.actions.retries = 0;
        cfg.actions.qmp = vec![QmpMapping {
            vm_id: vm_id.to_string(),
            vm: vm.to_string(),
            socket: "/run/libvirt/qemu/aegishv-test.monitor".to_string(),
        }];
        cfg
    }

    fn config_with_qmp_mappings(require_stable: bool, mappings: &[(&str, &str, &str)]) -> Config {
        let mut cfg = Config::default();
        cfg.identity.require_stable_qmp_match = require_stable;
        cfg.actions.retries = 0;
        cfg.actions.qmp = mappings
            .iter()
            .map(|(vm_id, vm, socket)| QmpMapping {
                vm_id: (*vm_id).to_string(),
                vm: (*vm).to_string(),
                socket: (*socket).to_string(),
            })
            .collect();
        cfg
    }

    fn temp_dir(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("aegishv-{label}-{}", crate::util::next_sequence()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn trusted_identity() -> IdentityInfo {
        IdentityInfo {
            sources: vec![
                IDENTITY_SOURCE_LIBVIRT_XML.to_string(),
                IDENTITY_SOURCE_START_TIME_VERIFIED.to_string(),
            ],
            confidence: IdentityConfidence::High,
            start_time_verified: true,
            ambiguous: false,
        }
    }

    fn pid_only_identity() -> IdentityInfo {
        IdentityInfo {
            sources: vec![
                IDENTITY_SOURCE_TRACE_COMM.to_string(),
                IDENTITY_SOURCE_FALLBACK_PID.to_string(),
            ],
            confidence: IdentityConfidence::Low,
            start_time_verified: false,
            ambiguous: false,
        }
    }

    fn low_trace_identity() -> IdentityInfo {
        IdentityInfo {
            sources: vec![IDENTITY_SOURCE_TRACE_COMM.to_string()],
            confidence: IdentityConfidence::Low,
            start_time_verified: false,
            ambiguous: false,
        }
    }

    fn medium_unverified_identity() -> IdentityInfo {
        IdentityInfo {
            sources: vec![IDENTITY_SOURCE_LIBVIRT_XML.to_string()],
            confidence: IdentityConfidence::Medium,
            start_time_verified: false,
            ambiguous: false,
        }
    }

    #[test]
    fn stable_qmp_match_uses_vm_id_when_required() {
        let cfg = config_with_qmp(true, "libvirt:111", "legacy-name");
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();

        let socket = dispatcher
            .qmp_socket_for(Some("libvirt:111"), "renamed-vm")
            .unwrap();

        assert_eq!(socket, "/run/libvirt/qemu/aegishv-test.monitor");
    }

    #[test]
    fn uuid_authoritative_qmp_mapping_wins_over_vm_name_fallback() {
        let cfg = config_with_qmp_mappings(
            false,
            &[
                (
                    "libvirt:111",
                    "legacy-name",
                    "/run/libvirt/qemu/uuid.monitor",
                ),
                ("", "legacy-name", "/run/libvirt/qemu/name.monitor"),
            ],
        );
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();

        let socket = dispatcher
            .qmp_socket_for(Some("libvirt:111"), "legacy-name")
            .unwrap();

        assert_eq!(socket, "/run/libvirt/qemu/uuid.monitor");
    }

    #[test]
    fn conflicting_uuid_authoritative_qmp_mappings_refuse_action() {
        let cfg = config_with_qmp_mappings(
            false,
            &[
                ("libvirt:111", "", "/run/libvirt/qemu/first.monitor"),
                (
                    "libvirt:111",
                    "legacy-name",
                    "/run/libvirt/qemu/second.monitor",
                ),
                ("", "legacy-name", "/run/libvirt/qemu/name.monitor"),
            ],
        );
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();

        let err = dispatcher
            .qmp_socket_for(Some("libvirt:111"), "legacy-name")
            .unwrap_err();

        assert_eq!(err.status(), "refused");
        assert!(err
            .into_detail()
            .contains("matched multiple actions.qmp sockets"));
    }

    #[test]
    fn vm_name_fallback_refuses_mapping_for_different_uuid() {
        let cfg = config_with_qmp_mappings(
            false,
            &[(
                "libvirt:222",
                "legacy-name",
                "/run/libvirt/qemu/wrong.monitor",
            )],
        );
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();

        let err = dispatcher
            .qmp_socket_for(Some("libvirt:111"), "legacy-name")
            .unwrap_err();

        assert_eq!(err.status(), "refused");
        assert!(err.into_detail().contains("entry for a different vm_id"));
    }

    #[test]
    fn stable_qmp_match_refuses_vm_name_fallback_without_vm_id() {
        let cfg = config_with_qmp(true, "", "legacy-name");
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();

        let err = dispatcher.qmp_socket_for(None, "legacy-name").unwrap_err();

        assert_eq!(err.status(), "refused");
        assert!(err.into_detail().contains("has no stable vm_id"));
    }

    #[test]
    fn stable_qmp_match_refuses_vm_name_fallback_when_vm_id_does_not_match() {
        let cfg = config_with_qmp(true, "libvirt:111", "legacy-name");
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();

        let err = dispatcher
            .qmp_socket_for(Some("libvirt:222"), "legacy-name")
            .unwrap_err();

        assert_eq!(err.status(), "refused");
        assert!(err
            .into_detail()
            .contains("no actions.qmp vm_id pattern matched"));
    }

    #[test]
    fn vm_name_qmp_fallback_still_works_when_requirement_is_disabled() {
        let cfg = config_with_qmp(false, "", "legacy-name");
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();

        let socket = dispatcher.qmp_socket_for(None, "legacy-name").unwrap();

        assert_eq!(socket, "/run/libvirt/qemu/aegishv-test.monitor");
    }

    #[test]
    fn ambiguous_vm_name_fallback_refuses_when_requirement_is_disabled() {
        let cfg = config_with_qmp_mappings(
            false,
            &[
                ("", "legacy-name", "/run/libvirt/qemu/first.monitor"),
                ("", "legacy-name", "/run/libvirt/qemu/second.monitor"),
            ],
        );
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();

        let err = dispatcher.qmp_socket_for(None, "legacy-name").unwrap_err();

        assert_eq!(err.status(), "refused");
        assert!(err
            .into_detail()
            .contains("VM-name fallback matched multiple actions.qmp sockets"));
    }

    #[test]
    fn ambiguous_vm_identity_refuses_qmp_even_when_name_fallback_is_allowed() {
        let cfg = config_with_qmp(false, "", "legacy-name");
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();

        let err = dispatcher
            .qmp_socket_for(Some("ambiguous:host-task:6262"), "legacy-name")
            .unwrap_err();

        assert_eq!(err.status(), "refused");
        assert!(err.into_detail().contains("VM identity is ambiguous"));
    }

    #[test]
    fn dispatcher_rejects_low_action_identity_confidence_threshold() {
        let mut cfg = config_with_qmp(true, "libvirt:111", "");
        cfg.identity.min_action_confidence = IdentityConfidence::Low;

        let err = match ActionDispatcher::new(&cfg) {
            Ok(_) => panic!("low identity confidence threshold must fail dispatcher setup"),
            Err(err) => err,
        };

        assert!(err.contains("low confidence cannot authorize QMP actions"));
    }

    #[test]
    fn refused_stable_qmp_match_emits_policy_audit_event() {
        let cfg = config_with_qmp(true, "", "legacy-name");
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();
        let metrics = Metrics::new().unwrap();

        let ev = dispatcher.run_action(
            &metrics,
            Some("rule-1"),
            "legacy-name",
            None,
            "pause_vm",
            None,
            None,
            Some(&trusted_identity()),
            &[],
            true,
        );
        let action = ev.action.as_ref().unwrap();

        assert!(!action.ok);
        assert_eq!(action.status, "refused");
        assert_eq!(ev.action_status.as_deref(), Some("refused"));
        assert_eq!(ev.decision.as_deref(), Some("refused"));
        assert_eq!(ev.severity, Severity::High);
        assert_eq!(action.decision, "refused");
        assert_eq!(action.result, "refused");
        assert_eq!(action.attempt, 1);
        assert_eq!(action.max_attempts, 1);
        assert_eq!(action.retry_count, 0);
        assert_eq!(action.timeout_ms, 2000);
        assert!(action.refused);
        assert!(!action.timed_out);
        assert_eq!(
            action.failure_class.as_deref(),
            Some("stable_identity_required")
        );
        assert!(action
            .detail
            .as_ref()
            .unwrap()
            .contains("identity.require_stable_qmp_match=true"));
        assert!(metrics.encode().contains(
            "aegishv_identity_qmp_safety_refusals_total{reason=\"stable_identity_required\"} 1"
        ));
    }

    #[test]
    fn ambiguous_identity_refusal_emits_policy_audit_event() {
        let cfg = config_with_qmp(false, "", "legacy-name");
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();
        let metrics = Metrics::new().unwrap();

        let ev = dispatcher.run_action(
            &metrics,
            Some("rule-ambiguous"),
            "legacy-name",
            Some("ambiguous:host-task:6262"),
            "pause_vm",
            None,
            None,
            None,
            &[],
            true,
        );
        let action = ev.action.as_ref().unwrap();

        assert!(!action.ok);
        assert_eq!(action.status, "refused");
        assert_eq!(action.result, "refused");
        assert_eq!(
            action.failure_class.as_deref(),
            Some("stable_identity_required")
        );
        assert!(action.refused);
        assert_eq!(action.attempt, 1);
        assert!(action
            .detail
            .as_ref()
            .unwrap()
            .contains("VM identity is ambiguous"));
    }

    #[test]
    fn conflicting_uuid_authoritative_mapping_emits_policy_audit_refusal() {
        let cfg = config_with_qmp_mappings(
            false,
            &[
                ("libvirt:111", "", "/run/libvirt/qemu/first.monitor"),
                ("libvirt:111", "", "/run/libvirt/qemu/second.monitor"),
            ],
        );
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();
        let metrics = Metrics::new().unwrap();

        let ev = dispatcher.run_action(
            &metrics,
            Some("rule-conflict"),
            "legacy-name",
            Some("libvirt:111"),
            "pause_vm",
            None,
            None,
            Some(&trusted_identity()),
            &[],
            true,
        );
        let action = ev.action.as_ref().unwrap();

        assert!(!action.ok);
        assert_eq!(action.status, "refused");
        assert_eq!(action.result, "refused");
        assert_eq!(
            action.failure_class.as_deref(),
            Some("stable_identity_required")
        );
        assert!(action.refused);
        assert!(action
            .detail
            .as_ref()
            .unwrap()
            .contains("matched multiple actions.qmp sockets"));
    }

    #[test]
    fn pid_only_identity_refuses_qmp_action_before_socket_selection() {
        let cfg = config_with_qmp(true, "libvirt:111", "");
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();
        let metrics = Metrics::new().unwrap();

        let ev = dispatcher.run_action(
            &metrics,
            Some("rule-pid-only"),
            "renamed-vm",
            Some("libvirt:111"),
            "pause_vm",
            None,
            None,
            Some(&pid_only_identity()),
            &[],
            true,
        );
        let action = ev.action.as_ref().unwrap();

        assert!(!action.ok);
        assert_eq!(action.status, "refused");
        assert_eq!(
            action.failure_class.as_deref(),
            Some("stable_identity_required")
        );
        assert!(action.refused);
        assert_eq!(action.attempt, 1);
        assert!(action
            .detail
            .as_ref()
            .unwrap()
            .contains("reason=pid_only_identity"));
        assert!(metrics.encode().contains(
            "aegishv_identity_qmp_safety_refusals_total{reason=\"pid_only_identity\"} 1"
        ));
    }

    #[test]
    fn medium_unverified_identity_refuses_qmp_action_at_medium_threshold() {
        let mut cfg = config_with_qmp(true, "libvirt:111", "");
        cfg.identity.min_action_confidence = IdentityConfidence::Medium;
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();
        let metrics = Metrics::new().unwrap();

        let ev = dispatcher.run_action(
            &metrics,
            Some("rule-unverified"),
            "renamed-vm",
            Some("libvirt:111"),
            "pause_vm",
            None,
            None,
            Some(&medium_unverified_identity()),
            &[],
            true,
        );
        let action = ev.action.as_ref().unwrap();

        assert!(!action.ok);
        assert_eq!(action.status, "refused");
        assert_eq!(
            action.failure_class.as_deref(),
            Some("stable_identity_required")
        );
        assert!(action
            .detail
            .as_ref()
            .unwrap()
            .contains("reason=unverified_identity"));
        assert!(metrics.encode().contains(
            "aegishv_identity_qmp_safety_refusals_total{reason=\"unverified_identity\"} 1"
        ));
    }

    #[test]
    fn low_confidence_identity_refuses_qmp_action() {
        let cfg = config_with_qmp(true, "libvirt:111", "");
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();
        let metrics = Metrics::new().unwrap();

        let ev = dispatcher.run_action(
            &metrics,
            Some("rule-low-confidence"),
            "renamed-vm",
            Some("libvirt:111"),
            "pause_vm",
            None,
            None,
            Some(&low_trace_identity()),
            &[],
            true,
        );
        let action = ev.action.as_ref().unwrap();

        assert!(!action.ok);
        assert_eq!(action.status, "refused");
        assert!(action
            .detail
            .as_ref()
            .unwrap()
            .contains("reason=low_confidence"));
        assert!(metrics
            .encode()
            .contains("aegishv_identity_qmp_safety_refusals_total{reason=\"low_confidence\"} 1"));
    }

    #[test]
    fn stale_identity_conflict_refuses_qmp_action() {
        let cfg = config_with_qmp(true, "libvirt:111", "");
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();
        let metrics = Metrics::new().unwrap();
        let tags = vec![
            "identity:conflict".to_string(),
            "identity_conflict:stale_cache".to_string(),
        ];

        let ev = dispatcher.run_action(
            &metrics,
            Some("rule-stale"),
            "renamed-vm",
            Some("libvirt:111"),
            "pause_vm",
            None,
            None,
            Some(&trusted_identity()),
            &tags,
            true,
        );
        let action = ev.action.as_ref().unwrap();

        assert!(!action.ok);
        assert_eq!(action.status, "refused");
        assert!(action
            .detail
            .as_ref()
            .unwrap()
            .contains("reason=stale_identity"));
        assert!(metrics
            .encode()
            .contains("aegishv_identity_qmp_safety_refusals_total{reason=\"stale_identity\"} 1"));
    }

    #[test]
    fn dry_run_allows_low_identity_for_qmp_backed_action() {
        let cfg = config_with_qmp(true, "libvirt:111", "");
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();
        let metrics = Metrics::new().unwrap();

        let ev = dispatcher.run_action(
            &metrics,
            Some("rule-dry-run"),
            "renamed-vm",
            Some("libvirt:111"),
            "pause_vm",
            None,
            None,
            Some(&pid_only_identity()),
            &[],
            false,
        );
        let action = ev.action.as_ref().unwrap();

        assert!(action.ok);
        assert_eq!(action.status, "dry_run");
        assert_eq!(action.attempt, 0);
        assert!(!action.refused);
    }

    #[test]
    fn manual_approval_allows_low_identity_without_qmp_execution() {
        let cfg = config_with_qmp(true, "libvirt:111", "");
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();
        let metrics = Metrics::new().unwrap();

        let ev = dispatcher.run_action(
            &metrics,
            Some("rule-manual"),
            "renamed-vm",
            Some("libvirt:111"),
            "manual_approval",
            None,
            None,
            Some(&pid_only_identity()),
            &[],
            true,
        );
        let action = ev.action.as_ref().unwrap();

        assert!(action.ok);
        assert_eq!(action.status, "accepted");
        assert_eq!(action.result, "accepted");
        assert!(!action.refused);
        assert!(action
            .detail
            .as_ref()
            .unwrap()
            .contains("no QMP command executed"));
    }

    #[test]
    fn unsafe_dump_path_emits_refused_action_audit_event() {
        let cfg = config_with_qmp(true, "libvirt:111", "");
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();
        let metrics = Metrics::new().unwrap();

        let ev = dispatcher.run_action(
            &metrics,
            Some("rule-1"),
            "renamed-vm",
            Some("libvirt:111"),
            "dump_guest_memory",
            Some("relative-dump.bin"),
            None,
            Some(&trusted_identity()),
            &[],
            true,
        );
        let action = ev.action.as_ref().unwrap();

        assert!(!action.ok);
        assert_eq!(action.status, "refused");
        assert_eq!(action.result, "refused");
        assert_eq!(action.failure_class.as_deref(), Some("unsafe_input"));
        assert_eq!(action.attempt, 1);
        assert!(action.refused);
        assert!(!action.timed_out);
        assert!(action
            .detail
            .as_ref()
            .unwrap()
            .contains("output_path must be absolute"));
    }

    #[test]
    fn dump_output_accepts_missing_file_under_safe_dump_root() {
        let root = temp_dir("dump-safe");
        let parent = root.join("vm-111");
        std::fs::create_dir_all(&parent).unwrap();
        let out = parent.join("guest.dump");

        validate_dump_output_path(&out, &root).unwrap();

        assert!(!out.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dump_output_rejects_parent_traversal_component() {
        let root = temp_dir("dump-parent");
        let out = root.join("vm-111").join("..").join("guest.dump");

        let err = validate_dump_output_path(&out, &root).unwrap_err();

        assert!(err.to_string().contains("must not contain .."));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dump_output_rejects_missing_parent_directory() {
        let root = temp_dir("dump-missing-parent");
        let out = root.join("missing").join("guest.dump");

        let err = validate_dump_output_path(&out, &root).unwrap_err();

        assert!(err.to_string().contains("parent does not exist"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dump_output_rejects_existing_file() {
        let root = temp_dir("dump-existing");
        let out = root.join("guest.dump");
        std::fs::write(&out, b"existing").unwrap();

        let err = validate_dump_output_path(&out, &root).unwrap_err();

        assert!(err.to_string().contains("already exists"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dump_output_rejects_path_outside_dump_root() {
        let root = temp_dir("dump-root");
        let outside = temp_dir("dump-outside");
        let out = outside.join("guest.dump");

        let err = validate_dump_output_path(&out, &root).unwrap_err();

        assert!(err.to_string().contains("must stay under dump_root"));
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(outside);
    }

    #[test]
    fn dump_root_must_be_existing_directory() {
        let root = temp_dir("dump-root-file");
        let root_file = root.join("not-a-directory");
        std::fs::write(&root_file, b"not a directory").unwrap();
        let out = root.join("guest.dump");

        let err = validate_dump_output_path(&out, &root_file).unwrap_err();

        assert!(err.to_string().contains("dump_root must be a directory"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unsupported_action_kind_emits_unsupported_action_audit_event() {
        let cfg = config_with_qmp(true, "libvirt:111", "");
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();
        let metrics = Metrics::new().unwrap();

        let ev = dispatcher.run_action(
            &metrics,
            Some("rule-1"),
            "renamed-vm",
            Some("libvirt:111"),
            "unsupported_action_kind",
            None,
            None,
            None,
            &[],
            true,
        );
        let action = ev.action.as_ref().unwrap();

        assert!(!action.ok);
        assert_eq!(action.status, "unsupported");
        assert_eq!(action.result, "unsupported");
        assert_eq!(action.failure_class.as_deref(), Some("unsupported_action"));
        assert_eq!(action.attempt, 1);
        assert_eq!(action.retry_count, 0);
        assert!(!action.refused);
        assert!(!action.timed_out);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::config::{
        Actions, Config, General, Identity, Journald, MetricsConfig, Pmu, Spool, Syslog,
    };
    use crate::metrics::Metrics;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;

    fn base_config(sock: &str) -> Config {
        Config {
            general: General::default(),
            allow: Default::default(),
            wx_allow: Default::default(),
            pmu: Pmu::default(),
            metrics: MetricsConfig::default(),
            spool: Spool::default(),
            syslog: Syslog::default(),
            journald: Journald::default(),
            identity: Identity::default(),
            actions: Actions {
                timeout_ms: 1000,
                retries: 0,
                dump_root: std::env::temp_dir().display().to_string(),
                qmp: vec![crate::config::QmpMapping {
                    vm_id: "libvirt:111".to_string(),
                    vm: String::new(),
                    socket: sock.to_string(),
                }],
            },
            policy: Default::default(),
            version: 1,
        }
    }

    fn config_with_retries(sock: &str, retries: u32) -> Config {
        let mut cfg = base_config(sock);
        cfg.actions.retries = retries;
        cfg
    }

    fn trusted_identity() -> IdentityInfo {
        IdentityInfo {
            sources: vec![
                IDENTITY_SOURCE_LIBVIRT_XML.to_string(),
                IDENTITY_SOURCE_START_TIME_VERIFIED.to_string(),
            ],
            confidence: IdentityConfidence::High,
            start_time_verified: true,
            ambiguous: false,
        }
    }

    fn spawn_qmp_server(sock: &Path, final_reply: &'static str) -> thread::JoinHandle<()> {
        let sock_path = sock.to_path_buf();
        thread::spawn(move || {
            let _ = std::fs::remove_file(&sock_path);
            let listener = UnixListener::bind(&sock_path).unwrap();
            let (mut stream, _) = listener.accept().unwrap();
            let reader_stream = stream.try_clone().unwrap();
            let mut reader = BufReader::new(reader_stream);
            stream
                .write_all(
                    br#"{"QMP":{"version":{"qemu":{"major":8}}}}
"#,
                )
                .unwrap();
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            stream
                .write_all(
                    br#"{"return":{}}
"#,
                )
                .unwrap();
            line.clear();
            reader.read_line(&mut line).unwrap();
            writeln!(stream, "{}", final_reply).unwrap();
        })
    }

    fn spawn_qmp_timeout_server(sock: &Path, timeout_ms: u64) -> thread::JoinHandle<()> {
        let sock_path = sock.to_path_buf();
        thread::spawn(move || {
            let _ = std::fs::remove_file(&sock_path);
            let listener = UnixListener::bind(&sock_path).unwrap();
            let (mut stream, _) = listener.accept().unwrap();
            let reader_stream = stream.try_clone().unwrap();
            let mut reader = BufReader::new(reader_stream);
            stream
                .write_all(
                    br#"{"QMP":{"version":{"qemu":{"major":8}}}}
"#,
                )
                .unwrap();
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            stream
                .write_all(
                    br#"{"return":{}}
"#,
                )
                .unwrap();
            line.clear();
            reader.read_line(&mut line).unwrap();
            thread::sleep(Duration::from_millis(timeout_ms.saturating_mul(3)));
        })
    }

    fn wait_for_socket(path: &Path) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if path.exists() {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("QMP test socket was not created: {}", path.display());
    }

    #[test]
    fn qmp_pause_action_success_by_vm_id() {
        let dir =
            std::env::temp_dir().join(format!("aegishv-qmp-{}", crate::util::next_sequence()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("qmp.sock");
        let server = spawn_qmp_server(&sock, "{\"return\":{}}");
        wait_for_socket(&sock);
        let cfg = base_config(sock.to_str().unwrap());
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();
        let metrics = Metrics::new().unwrap();
        let ev = dispatcher.run_action(
            &metrics,
            Some("rule-1"),
            "renamed-vm",
            Some("libvirt:111"),
            "pause_vm",
            None,
            None,
            Some(&trusted_identity()),
            &[],
            true,
        );
        assert!(ev.action.as_ref().unwrap().ok);
        let action = ev.action.as_ref().unwrap();
        assert_eq!(action.status, "completed");
        assert_eq!(action.decision, "executed");
        assert_eq!(action.result, "completed");
        assert_eq!(action.attempt, 1);
        assert_eq!(action.max_attempts, 1);
        assert_eq!(action.retry_count, 0);
        assert_eq!(action.timeout_ms, 1000);
        assert!(!action.refused);
        assert!(!action.timed_out);
        assert_eq!(action.failure_class, None);
        server.join().unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn qmp_connect_failure_records_retry_audit_metadata() {
        let dir = std::env::temp_dir().join(format!(
            "aegishv-qmp-missing-{}",
            crate::util::next_sequence()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("missing.sock");
        let cfg = config_with_retries(sock.to_str().unwrap(), 2);
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();
        let metrics = Metrics::new().unwrap();

        let ev = dispatcher.run_action(
            &metrics,
            Some("rule-1"),
            "renamed-vm",
            Some("libvirt:111"),
            "pause_vm",
            None,
            None,
            Some(&trusted_identity()),
            &[],
            true,
        );

        let action = ev.action.as_ref().unwrap();
        assert!(!action.ok);
        assert_eq!(action.status, "error");
        assert_eq!(action.result, "error");
        assert_eq!(action.attempt, 3);
        assert_eq!(action.max_attempts, 3);
        assert_eq!(action.retry_count, 2);
        assert!(!action.refused);
        assert!(!action.timed_out);
        assert_eq!(action.failure_class.as_deref(), Some("qmp_error"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn qmp_timeout_records_timeout_audit_metadata() {
        let dir = std::env::temp_dir().join(format!(
            "aegishv-qmp-timeout-{}",
            crate::util::next_sequence()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("qmp.sock");
        let server = spawn_qmp_timeout_server(&sock, 100);
        wait_for_socket(&sock);
        let mut cfg = base_config(sock.to_str().unwrap());
        cfg.actions.timeout_ms = 100;
        let dispatcher = ActionDispatcher::new(&cfg).unwrap();
        let metrics = Metrics::new().unwrap();

        let ev = dispatcher.run_action(
            &metrics,
            Some("rule-1"),
            "renamed-vm",
            Some("libvirt:111"),
            "pause_vm",
            None,
            None,
            Some(&trusted_identity()),
            &[],
            true,
        );

        let action = ev.action.as_ref().unwrap();
        assert!(!action.ok);
        assert_eq!(action.status, "timeout");
        assert_eq!(action.result, "timeout");
        assert_eq!(action.attempt, 1);
        assert_eq!(action.max_attempts, 1);
        assert_eq!(action.retry_count, 0);
        assert_eq!(action.timeout_ms, 100);
        assert!(action.timed_out);
        assert!(!action.refused);
        assert_eq!(action.failure_class.as_deref(), Some("timeout"));
        server.join().unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dump_output_rejects_existing_symlink() {
        let dir =
            std::env::temp_dir().join(format!("aegishv-dump-{}", crate::util::next_sequence()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("real.bin");
        let link = dir.join("dump.bin");
        std::fs::write(&target, b"x").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(validate_dump_output_path(&link, &dir).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dump_output_rejects_symlink_parent_ancestor() {
        let dir = std::env::temp_dir().join(format!(
            "aegishv-dump-parent-link-{}",
            crate::util::next_sequence()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let real_parent = dir.join("real-parent");
        let linked_parent = dir.join("linked-parent");
        std::fs::create_dir_all(&real_parent).unwrap();
        std::os::unix::fs::symlink(&real_parent, &linked_parent).unwrap();
        let out = linked_parent.join("guest.dump");

        let err = validate_dump_output_path(&out, &dir).unwrap_err();

        assert!(err.to_string().contains("symlink ancestor"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dump_root_rejects_symlink_root() {
        let dir = std::env::temp_dir().join(format!(
            "aegishv-dump-root-link-{}",
            crate::util::next_sequence()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let real_root = dir.join("real-root");
        let linked_root = dir.join("linked-root");
        std::fs::create_dir_all(&real_root).unwrap();
        std::os::unix::fs::symlink(&real_root, &linked_root).unwrap();
        let out = real_root.join("guest.dump");

        let err = validate_dump_output_path(&out, &linked_root).unwrap_err();

        assert!(err.to_string().contains("dump_root must not be a symlink"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
