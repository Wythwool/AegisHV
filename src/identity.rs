use crate::config::Identity;
use crate::event::{Category, Event, IdentityConfidence, IdentityInfo, Severity};
use crate::util::{json_str, now_rfc3339};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const IDENTITY_CONFLICT_COOLDOWN: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmIdentity {
    pub host_pid: i32,
    pub host_start_time_ticks: Option<u64>,
    pub vm_id: String,
    pub vm_name: Option<String>,
    pub libvirt_uuid: Option<String>,
    pub cgroup_unit: Option<String>,
    pub qmp_socket: Option<String>,
    pub vcpu_id: Option<i32>,
    pub vcpu_ambiguous: bool,
    pub ambiguous: bool,
    pub identity_sources: Vec<String>,
    pub identity_confidence: IdentityConfidence,
    pub start_time_verified: bool,
    pub identity_conflict: Option<IdentityConflict>,
}

#[derive(Debug, Clone)]
struct CachedIdentity {
    seen_at: Instant,
    start_time_ticks: Option<u64>,
    value: VmIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibvirtDomain {
    pub name: String,
    pub uuid: String,
    pub qmp_socket: Option<String>,
    pub qemu_pids: Vec<i32>,
    pub qemu_tids: Vec<i32>,
    pub vcpu_threads: Vec<QemuVcpuThread>,
    pub qemu_task_identities: Vec<HostTaskIdentity>,
    pub source: LibvirtDomainSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibvirtDomainSource {
    Xml,
    Lifecycle,
}

impl LibvirtDomainSource {
    fn identity_source(self) -> &'static str {
        match self {
            Self::Xml => IDENTITY_SOURCE_LIBVIRT_XML,
            Self::Lifecycle => IDENTITY_SOURCE_LIBVIRT_LIFECYCLE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostTaskIdentity {
    pub task_id: i32,
    pub start_time_ticks: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QemuVcpuThread {
    pub tid: i32,
    pub vcpu_id: i32,
}

#[derive(Debug, Clone, Default)]
pub struct LibvirtDomainDiscovery {
    domains: Vec<LibvirtDomain>,
}

pub const IDENTITY_SOURCE_TRACE_COMM: &str = "trace_comm";
pub const IDENTITY_SOURCE_PROC_CMDLINE: &str = "proc_cmdline";
pub const IDENTITY_SOURCE_PROC_CGROUP: &str = "proc_cgroup";
pub const IDENTITY_SOURCE_LIBVIRT_XML: &str = "libvirt_xml";
pub const IDENTITY_SOURCE_QMP_SOCKET_HINT: &str = "qmp_socket_hint";
pub const IDENTITY_SOURCE_LIBVIRT_LIFECYCLE: &str = "libvirt_lifecycle";
pub const IDENTITY_SOURCE_FALLBACK_PID: &str = "fallback_pid";
pub const IDENTITY_SOURCE_AMBIGUOUS: &str = "ambiguous";
pub const IDENTITY_SOURCE_START_TIME_VERIFIED: &str = "start_time_verified";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityConflict {
    pub task_id: i32,
    pub reason: IdentityConflictReason,
    pub ambiguous: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdentityConflictReason {
    MultipleDomains,
    PidReuse,
    StartTimeUnverified,
    StaleCache,
    ProcCgroupMismatch,
    LibvirtUuidMismatch,
    QmpSocketMismatch,
    LibvirtNameMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityCacheResult {
    Hit,
    Miss,
    Refusal,
}

#[derive(Debug, Clone)]
pub struct IdentityResolution {
    pub identity: VmIdentity,
    pub cache_result: IdentityCacheResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmInventorySnapshot {
    pub status: String,
    pub source: String,
    pub freshness: String,
    pub vm_count: usize,
    pub degraded: bool,
    pub vms: Vec<VmInventoryVm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmInventoryVm {
    pub vm_id: String,
    pub vm_uuid: String,
    pub vm_name: String,
    pub source: String,
    pub known_host_tasks: Vec<VmInventoryHostTask>,
    pub vcpu_mappings: Vec<VmInventoryVcpuMapping>,
    pub qmp: VmInventoryQmp,
    pub identity: IdentityInfo,
    pub ambiguous: bool,
    pub conflict: Option<VmInventoryConflict>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmInventoryHostTask {
    pub kind: String,
    pub id: i32,
    pub start_time_ticks: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmInventoryVcpuMapping {
    pub host_tid: i32,
    pub vcpu_id: i32,
    pub start_time_ticks: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmInventoryQmp {
    pub present: bool,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmInventoryConflict {
    pub reason: String,
    pub ambiguous: bool,
}

impl IdentityConflictReason {
    pub const ALL: [Self; 8] = [
        Self::MultipleDomains,
        Self::PidReuse,
        Self::StartTimeUnverified,
        Self::StaleCache,
        Self::ProcCgroupMismatch,
        Self::LibvirtUuidMismatch,
        Self::QmpSocketMismatch,
        Self::LibvirtNameMismatch,
    ];
    pub const COUNT: usize = Self::ALL.len();

    pub fn index(self) -> usize {
        match self {
            Self::MultipleDomains => 0,
            Self::PidReuse => 1,
            Self::StartTimeUnverified => 2,
            Self::StaleCache => 3,
            Self::ProcCgroupMismatch => 4,
            Self::LibvirtUuidMismatch => 5,
            Self::QmpSocketMismatch => 6,
            Self::LibvirtNameMismatch => 7,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::MultipleDomains => "multiple_domains",
            Self::PidReuse => "pid_reuse",
            Self::StartTimeUnverified => "start_time_unverified",
            Self::StaleCache => "stale_cache",
            Self::ProcCgroupMismatch => "proc_cgroup_mismatch",
            Self::LibvirtUuidMismatch => "libvirt_uuid_mismatch",
            Self::QmpSocketMismatch => "qmp_socket_mismatch",
            Self::LibvirtNameMismatch => "libvirt_name_mismatch",
        }
    }

    fn vm_id_suffix(self) -> Option<&'static str> {
        match self {
            Self::MultipleDomains => None,
            Self::PidReuse => Some("pid-reuse"),
            Self::StartTimeUnverified => Some("start-time-unverified"),
            Self::StaleCache => Some("stale-cache"),
            Self::ProcCgroupMismatch => Some("proc-cgroup-mismatch"),
            Self::LibvirtUuidMismatch => Some("libvirt-uuid-mismatch"),
            Self::QmpSocketMismatch => Some("qmp-socket-mismatch"),
            Self::LibvirtNameMismatch => Some("libvirt-name-mismatch"),
        }
    }

    fn severity(self) -> Severity {
        match self {
            Self::LibvirtNameMismatch | Self::StaleCache => Severity::Low,
            _ => Severity::Medium,
        }
    }
}

impl IdentityCacheResult {
    pub const ALL: [Self; 3] = [Self::Hit, Self::Miss, Self::Refusal];
    pub const COUNT: usize = Self::ALL.len();

    pub fn index(self) -> usize {
        match self {
            Self::Hit => 0,
            Self::Miss => 1,
            Self::Refusal => 2,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::Miss => "miss",
            Self::Refusal => "refusal",
        }
    }
}

impl VmInventorySnapshot {
    pub fn empty() -> Self {
        Self {
            status: "ok".to_string(),
            source: "none".to_string(),
            freshness: "none".to_string(),
            vm_count: 0,
            degraded: false,
            vms: Vec::new(),
        }
    }

    pub fn disabled() -> Self {
        Self {
            status: "disabled".to_string(),
            source: "disabled".to_string(),
            freshness: "none".to_string(),
            vm_count: 0,
            degraded: false,
            vms: Vec::new(),
        }
    }

    fn degraded_empty() -> Self {
        Self {
            status: "degraded".to_string(),
            source: "none".to_string(),
            freshness: "none".to_string(),
            vm_count: 0,
            degraded: true,
            vms: Vec::new(),
        }
    }

    pub fn to_json(&self) -> String {
        let vms = self
            .vms
            .iter()
            .map(inventory_vm_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"status\":{},\"source\":{},\"freshness\":{},\"vm_count\":{},\"degraded\":{},\"vms\":[{}]}}",
            json_str(&self.status),
            json_str(&self.source),
            json_str(&self.freshness),
            self.vm_count,
            self.degraded,
            vms
        )
    }
}

impl Default for VmInventorySnapshot {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct IdentityEnrichment {
    pub conflict_event: Option<Event>,
    pub cache_result: Option<IdentityCacheResult>,
    pub confidence: Option<IdentityConfidence>,
    pub ambiguous: bool,
    pub conflict_reason: Option<IdentityConflictReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct IdentityConflictKey {
    task_id: i32,
    reason: IdentityConflictReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibvirtLifecycleKind {
    Started,
    Stopped,
    Paused,
    Resumed,
    Migrated,
}

impl LibvirtLifecycleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Stopped => "stopped",
            Self::Paused => "paused",
            Self::Resumed => "resumed",
            Self::Migrated => "migrated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibvirtLifecycleEvent {
    pub kind: LibvirtLifecycleKind,
    pub uuid: String,
    pub name: Option<String>,
    pub qemu_pid: Option<i32>,
    pub qemu_tids: Vec<i32>,
    pub vcpu_threads: Vec<QemuVcpuThread>,
    pub qmp_socket: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibvirtLifecycleError {
    UnsupportedKind(String),
    MalformedEvent(String),
    Source(String),
}

impl fmt::Display for LibvirtLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedKind(kind) => {
                write!(f, "unsupported libvirt lifecycle event kind: {kind}")
            }
            Self::MalformedEvent(detail) => {
                write!(f, "malformed libvirt lifecycle event: {detail}")
            }
            Self::Source(detail) => write!(f, "libvirt lifecycle source error: {detail}"),
        }
    }
}

impl std::error::Error for LibvirtLifecycleError {}

impl TryFrom<&str> for LibvirtLifecycleKind {
    type Error = LibvirtLifecycleError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.trim().to_ascii_lowercase().as_str() {
            "start" | "started" => Ok(Self::Started),
            "stop" | "stopped" => Ok(Self::Stopped),
            "pause" | "paused" => Ok(Self::Paused),
            "resume" | "resumed" => Ok(Self::Resumed),
            "migrate" | "migrated" => Ok(Self::Migrated),
            other => Err(LibvirtLifecycleError::UnsupportedKind(other.to_string())),
        }
    }
}

pub trait LibvirtLifecycleSource {
    fn next_event(&mut self) -> Result<Option<LibvirtLifecycleEvent>, LibvirtLifecycleError>;
}

pub struct QueuedLibvirtLifecycleSource {
    events: VecDeque<Result<LibvirtLifecycleEvent, LibvirtLifecycleError>>,
}

impl QueuedLibvirtLifecycleSource {
    pub fn new(events: Vec<Result<LibvirtLifecycleEvent, LibvirtLifecycleError>>) -> Self {
        Self {
            events: VecDeque::from(events),
        }
    }
}

impl LibvirtLifecycleSource for QueuedLibvirtLifecycleSource {
    fn next_event(&mut self) -> Result<Option<LibvirtLifecycleEvent>, LibvirtLifecycleError> {
        match self.events.pop_front() {
            Some(Ok(event)) => Ok(Some(event)),
            Some(Err(err)) => Err(err),
            None => Ok(None),
        }
    }
}

impl LibvirtLifecycleEvent {
    fn task_ids(&self) -> Vec<i32> {
        let mut tasks = Vec::with_capacity(1 + self.qemu_tids.len());
        if let Some(pid) = self.qemu_pid {
            tasks.push(pid);
        }
        tasks.extend(self.qemu_tids.iter().copied());
        tasks.extend(self.vcpu_threads.iter().map(|thread| thread.tid));
        tasks.sort_unstable();
        tasks.dedup();
        tasks
    }

    fn to_domain(&self) -> Option<LibvirtDomain> {
        let name = self.name.as_ref()?.trim();
        let uuid = self.uuid.trim();
        if name.is_empty() || uuid.is_empty() {
            return None;
        }
        let tasks = self.task_ids();
        if tasks.is_empty() {
            return None;
        }
        let qemu_pids = self.qemu_pid.into_iter().collect::<Vec<_>>();
        Some(LibvirtDomain {
            name: name.to_string(),
            uuid: uuid.to_string(),
            qmp_socket: self.qmp_socket.clone(),
            qemu_pids,
            qemu_tids: self.qemu_tids.clone(),
            vcpu_threads: self.vcpu_threads.clone(),
            qemu_task_identities: self
                .task_ids()
                .into_iter()
                .map(|task_id| HostTaskIdentity {
                    task_id,
                    start_time_ticks: None,
                })
                .collect(),
            source: LibvirtDomainSource::Lifecycle,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibvirtDiscoveryError {
    MissingDirectory(PathBuf),
    ReadDirectory {
        path: PathBuf,
        source: String,
    },
    ReadDomain {
        path: PathBuf,
        source: String,
    },
    MalformedDomain {
        path: PathBuf,
        detail: String,
    },
    AmbiguousTask {
        task_id: i32,
        domains: Vec<String>,
    },
    PidReuseDetected {
        task_id: i32,
        observed_start_time_ticks: Option<u64>,
        domains: Vec<String>,
    },
    UnverifiedTaskIdentity {
        task_id: i32,
        domains: Vec<String>,
    },
}

impl fmt::Display for LibvirtDiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDirectory(path) => write!(
                f,
                "libvirt XML discovery directory does not exist or is not a directory: {}",
                path.display()
            ),
            Self::ReadDirectory { path, source } => {
                write!(
                    f,
                    "read libvirt XML discovery directory {}: {source}",
                    path.display()
                )
            }
            Self::ReadDomain { path, source } => {
                write!(f, "read libvirt domain XML {}: {source}", path.display())
            }
            Self::MalformedDomain { path, detail } => {
                write!(f, "parse libvirt domain XML {}: {detail}", path.display())
            }
            Self::AmbiguousTask { task_id, domains } => write!(
                f,
                "libvirt XML discovery found multiple domains for host task {task_id}: {}",
                domains.join(",")
            ),
            Self::PidReuseDetected {
                task_id,
                observed_start_time_ticks,
                domains,
            } => write!(
                f,
                "libvirt XML discovery rejected host task {task_id}: observed start_time_ticks {:?} does not match mapped process identity for {}",
                observed_start_time_ticks,
                domains.join(",")
            ),
            Self::UnverifiedTaskIdentity { task_id, domains } => write!(
                f,
                "libvirt XML discovery cannot verify host task {task_id} with process start_time_ticks for {}",
                domains.join(",")
            ),
        }
    }
}

impl std::error::Error for LibvirtDiscoveryError {}

impl LibvirtDomainDiscovery {
    pub fn from_dir(path: &Path) -> Result<Self, LibvirtDiscoveryError> {
        if !path.is_dir() {
            return Err(LibvirtDiscoveryError::MissingDirectory(path.to_path_buf()));
        }

        let entries =
            std::fs::read_dir(path).map_err(|e| LibvirtDiscoveryError::ReadDirectory {
                path: path.to_path_buf(),
                source: e.to_string(),
            })?;
        let mut domains = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| LibvirtDiscoveryError::ReadDirectory {
                path: path.to_path_buf(),
                source: e.to_string(),
            })?;
            let domain_path = entry.path();
            if !domain_path.is_file()
                || domain_path.extension().and_then(|value| value.to_str()) != Some("xml")
            {
                continue;
            }
            let xml = std::fs::read_to_string(&domain_path).map_err(|e| {
                LibvirtDiscoveryError::ReadDomain {
                    path: domain_path.clone(),
                    source: e.to_string(),
                }
            })?;
            domains.push(parse_libvirt_domain_xml(&domain_path, &xml)?);
        }
        Ok(Self { domains })
    }

    pub fn from_domains(domains: Vec<LibvirtDomain>) -> Self {
        Self { domains }
    }

    pub fn inventory_snapshot(&self) -> VmInventorySnapshot {
        if self.domains.is_empty() {
            return VmInventorySnapshot::empty();
        }

        let task_counts = inventory_task_counts(&self.domains);
        let mut vms = self
            .domains
            .iter()
            .map(|domain| inventory_vm(domain, &task_counts))
            .collect::<Vec<_>>();
        vms.sort_by(|left, right| left.vm_uuid.cmp(&right.vm_uuid));
        let degraded = vms.iter().any(|vm| vm.ambiguous || vm.conflict.is_some());
        let (source, freshness) = inventory_source_and_freshness(&self.domains);
        VmInventorySnapshot {
            status: if degraded { "degraded" } else { "ok" }.to_string(),
            source: source.to_string(),
            freshness: freshness.to_string(),
            vm_count: vms.len(),
            degraded,
            vms,
        }
    }

    pub fn upsert_domain(&mut self, domain: LibvirtDomain) {
        let mut tasks = domain_task_ids(&domain);
        tasks.sort_unstable();
        tasks.dedup();
        for existing in &mut self.domains {
            if existing.uuid == domain.uuid {
                continue;
            }
            existing
                .qemu_pids
                .retain(|pid| tasks.binary_search(pid).is_err());
            existing
                .qemu_tids
                .retain(|tid| tasks.binary_search(tid).is_err());
            existing
                .vcpu_threads
                .retain(|thread| tasks.binary_search(&thread.tid).is_err());
            existing
                .qemu_task_identities
                .retain(|identity| tasks.binary_search(&identity.task_id).is_err());
        }
        self.domains.retain(|existing| {
            existing.uuid == domain.uuid
                || !existing.qemu_pids.is_empty()
                || !existing.qemu_tids.is_empty()
                || !existing.vcpu_threads.is_empty()
                || !existing.qemu_task_identities.is_empty()
        });
        if let Some(existing) = self
            .domains
            .iter_mut()
            .find(|existing| existing.uuid == domain.uuid)
        {
            *existing = domain;
        } else {
            self.domains.push(domain);
        }
    }

    pub fn remove_domain_uuid(&mut self, uuid: &str) -> bool {
        let before = self.domains.len();
        self.domains.retain(|domain| domain.uuid != uuid);
        self.domains.len() != before
    }

    pub fn lookup_task(
        &self,
        task_id: i32,
    ) -> Result<Option<&LibvirtDomain>, LibvirtDiscoveryError> {
        self.lookup_task_with_start_time(task_id, None)
    }

    pub fn lookup_task_with_start_time(
        &self,
        task_id: i32,
        observed_start_time_ticks: Option<u64>,
    ) -> Result<Option<&LibvirtDomain>, LibvirtDiscoveryError> {
        let matches = self
            .domains
            .iter()
            .filter(|domain| domain_has_task(domain, task_id))
            .filter(|domain| {
                task_start_time_matches(domain, task_id, observed_start_time_ticks)
                    == TaskStartTimeMatch::Match
            })
            .collect::<Vec<_>>();
        if matches.is_empty() {
            let unsafe_matches = self
                .domains
                .iter()
                .filter(|domain| domain_has_task(domain, task_id))
                .filter_map(|domain| {
                    match task_start_time_matches(domain, task_id, observed_start_time_ticks) {
                        TaskStartTimeMatch::PidReuseDetected => {
                            Some((domain, TaskStartTimeMatch::PidReuseDetected))
                        }
                        TaskStartTimeMatch::Unverified => {
                            Some((domain, TaskStartTimeMatch::Unverified))
                        }
                        TaskStartTimeMatch::Match => None,
                    }
                })
                .collect::<Vec<_>>();
            if unsafe_matches
                .iter()
                .any(|(_, status)| *status == TaskStartTimeMatch::PidReuseDetected)
            {
                return Err(LibvirtDiscoveryError::PidReuseDetected {
                    task_id,
                    observed_start_time_ticks,
                    domains: unsafe_matches
                        .iter()
                        .map(|(domain, _)| format!("{}:{}", domain.uuid, domain.name))
                        .collect(),
                });
            }
            if !unsafe_matches.is_empty() {
                return Err(LibvirtDiscoveryError::UnverifiedTaskIdentity {
                    task_id,
                    domains: unsafe_matches
                        .iter()
                        .map(|(domain, _)| format!("{}:{}", domain.uuid, domain.name))
                        .collect(),
                });
            }
        }
        match matches.as_slice() {
            [] => Ok(None),
            [domain] => Ok(Some(*domain)),
            many => Err(LibvirtDiscoveryError::AmbiguousTask {
                task_id,
                domains: many
                    .iter()
                    .map(|domain| format!("{}:{}", domain.uuid, domain.name))
                    .collect(),
            }),
        }
    }
}

fn domain_task_ids(domain: &LibvirtDomain) -> Vec<i32> {
    let mut tasks = Vec::with_capacity(
        domain.qemu_pids.len()
            + domain.qemu_tids.len()
            + domain.vcpu_threads.len()
            + domain.qemu_task_identities.len(),
    );
    tasks.extend(domain.qemu_pids.iter().copied());
    tasks.extend(domain.qemu_tids.iter().copied());
    tasks.extend(domain.vcpu_threads.iter().map(|thread| thread.tid));
    tasks.extend(
        domain
            .qemu_task_identities
            .iter()
            .map(|identity| identity.task_id),
    );
    tasks
}

fn domain_has_task(domain: &LibvirtDomain, task_id: i32) -> bool {
    domain.qemu_pids.contains(&task_id)
        || domain.qemu_tids.contains(&task_id)
        || domain
            .vcpu_threads
            .iter()
            .any(|thread| thread.tid == task_id)
        || domain
            .qemu_task_identities
            .iter()
            .any(|identity| identity.task_id == task_id)
}

fn inventory_task_counts(domains: &[LibvirtDomain]) -> HashMap<i32, usize> {
    let mut counts = HashMap::new();
    for domain in domains {
        let mut tasks = domain_task_ids(domain);
        tasks.sort_unstable();
        tasks.dedup();
        for task_id in tasks {
            *counts.entry(task_id).or_insert(0) += 1;
        }
    }
    counts
}

fn inventory_source_and_freshness(domains: &[LibvirtDomain]) -> (&'static str, &'static str) {
    let has_xml = domains
        .iter()
        .any(|domain| domain.source == LibvirtDomainSource::Xml);
    let has_lifecycle = domains
        .iter()
        .any(|domain| domain.source == LibvirtDomainSource::Lifecycle);
    match (has_xml, has_lifecycle) {
        (true, true) => ("mixed", "mixed"),
        (true, false) => ("libvirt_xml", "file_backed_snapshot"),
        (false, true) => ("libvirt_lifecycle", "mockable_lifecycle_state"),
        (false, false) => ("none", "none"),
    }
}

fn inventory_vm(domain: &LibvirtDomain, task_counts: &HashMap<i32, usize>) -> VmInventoryVm {
    let duplicate_task = domain_task_ids(domain)
        .into_iter()
        .any(|task_id| task_counts.get(&task_id).copied().unwrap_or(0) > 1);
    let ambiguous = duplicate_task;
    let conflict = duplicate_task.then(|| VmInventoryConflict {
        reason: IdentityConflictReason::MultipleDomains.as_str().to_string(),
        ambiguous: true,
    });
    let mut sources = vec![domain.source.identity_source().to_string()];
    if domain.qmp_socket.is_some() {
        push_identity_source(&mut sources, IDENTITY_SOURCE_QMP_SOCKET_HINT);
    }
    if ambiguous {
        push_identity_source(&mut sources, IDENTITY_SOURCE_AMBIGUOUS);
    }
    let identity = IdentityInfo {
        sources,
        confidence: if ambiguous {
            IdentityConfidence::Low
        } else {
            IdentityConfidence::Medium
        },
        start_time_verified: false,
        ambiguous,
    };
    VmInventoryVm {
        vm_id: format!("libvirt:{}", domain.uuid),
        vm_uuid: domain.uuid.clone(),
        vm_name: domain.name.clone(),
        source: domain.source.identity_source().to_string(),
        known_host_tasks: inventory_host_tasks(domain),
        vcpu_mappings: inventory_vcpu_mappings(domain),
        qmp: VmInventoryQmp {
            present: domain.qmp_socket.is_some(),
            status: if domain.qmp_socket.is_some() {
                "configured"
            } else {
                "missing"
            }
            .to_string(),
        },
        identity,
        ambiguous,
        conflict,
    }
}

fn inventory_host_tasks(domain: &LibvirtDomain) -> Vec<VmInventoryHostTask> {
    let mut tasks = Vec::new();
    for pid in &domain.qemu_pids {
        push_inventory_host_task(
            &mut tasks,
            "pid",
            *pid,
            inventory_start_time_for(domain, *pid),
        );
    }
    for tid in &domain.qemu_tids {
        push_inventory_host_task(
            &mut tasks,
            "tid",
            *tid,
            inventory_start_time_for(domain, *tid),
        );
    }
    for thread in &domain.vcpu_threads {
        push_inventory_host_task(
            &mut tasks,
            "tid",
            thread.tid,
            inventory_start_time_for(domain, thread.tid),
        );
    }
    for identity in &domain.qemu_task_identities {
        if tasks.iter().all(|task| task.id != identity.task_id) {
            push_inventory_host_task(
                &mut tasks,
                "task",
                identity.task_id,
                identity.start_time_ticks,
            );
        }
    }
    tasks.sort_by(|left, right| left.id.cmp(&right.id).then(left.kind.cmp(&right.kind)));
    tasks
}

fn push_inventory_host_task(
    tasks: &mut Vec<VmInventoryHostTask>,
    kind: &str,
    id: i32,
    start_time_ticks: Option<u64>,
) {
    if let Some(existing) = tasks
        .iter_mut()
        .find(|task| task.kind == kind && task.id == id)
    {
        if existing.start_time_ticks.is_none() {
            existing.start_time_ticks = start_time_ticks;
        }
    } else {
        tasks.push(VmInventoryHostTask {
            kind: kind.to_string(),
            id,
            start_time_ticks,
        });
    }
}

fn inventory_vcpu_mappings(domain: &LibvirtDomain) -> Vec<VmInventoryVcpuMapping> {
    let mut mappings = domain
        .vcpu_threads
        .iter()
        .map(|thread| VmInventoryVcpuMapping {
            host_tid: thread.tid,
            vcpu_id: thread.vcpu_id,
            start_time_ticks: inventory_start_time_for(domain, thread.tid),
        })
        .collect::<Vec<_>>();
    mappings.sort_by(|left, right| {
        left.host_tid
            .cmp(&right.host_tid)
            .then(left.vcpu_id.cmp(&right.vcpu_id))
    });
    mappings
}

fn inventory_start_time_for(domain: &LibvirtDomain, task_id: i32) -> Option<u64> {
    domain
        .qemu_task_identities
        .iter()
        .find_map(|identity| (identity.task_id == task_id).then_some(identity.start_time_ticks)?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskStartTimeMatch {
    Match,
    PidReuseDetected,
    Unverified,
}

fn task_start_time_matches(
    domain: &LibvirtDomain,
    task_id: i32,
    observed_start_time_ticks: Option<u64>,
) -> TaskStartTimeMatch {
    let matches = domain
        .qemu_task_identities
        .iter()
        .filter(|identity| identity.task_id == task_id)
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return if observed_start_time_ticks.is_some() {
            TaskStartTimeMatch::Unverified
        } else {
            TaskStartTimeMatch::Match
        };
    }

    let mut has_unversioned_mapping = false;
    for identity in matches {
        match (identity.start_time_ticks, observed_start_time_ticks) {
            (Some(expected), Some(observed)) if expected == observed => {
                return TaskStartTimeMatch::Match;
            }
            (Some(_), Some(_)) => return TaskStartTimeMatch::PidReuseDetected,
            (Some(_), None) => return TaskStartTimeMatch::Unverified,
            (None, _) => has_unversioned_mapping = true,
        }
    }

    if has_unversioned_mapping && observed_start_time_ticks.is_none() {
        TaskStartTimeMatch::Match
    } else {
        TaskStartTimeMatch::Unverified
    }
}

pub struct VmIdentityResolver {
    enable: bool,
    live_host_lookup: bool,
    ttl: Duration,
    qmp_socket_dirs: Vec<PathBuf>,
    libvirt: Mutex<Option<LibvirtDomainDiscovery>>,
    cache: Mutex<HashMap<i32, CachedIdentity>>,
    last_conflicts: Mutex<HashMap<IdentityConflictKey, Instant>>,
}

impl VmIdentityResolver {
    pub fn new(cfg: Identity) -> Result<Self, LibvirtDiscoveryError> {
        Self::with_live_host_lookup(cfg, true)
    }

    pub fn deterministic_replay(cfg: Identity) -> Result<Self, LibvirtDiscoveryError> {
        Self::with_live_host_lookup(cfg, false)
    }

    fn with_live_host_lookup(
        cfg: Identity,
        live_host_lookup: bool,
    ) -> Result<Self, LibvirtDiscoveryError> {
        let libvirt = if !live_host_lookup || cfg.libvirt_xml_dir.trim().is_empty() {
            None
        } else {
            Some(LibvirtDomainDiscovery::from_dir(Path::new(
                &cfg.libvirt_xml_dir,
            ))?)
        };
        Ok(Self {
            enable: cfg.enable,
            live_host_lookup,
            ttl: Duration::from_millis(cfg.cache_ms),
            qmp_socket_dirs: cfg.qmp_socket_dirs.into_iter().map(PathBuf::from).collect(),
            libvirt: Mutex::new(libvirt),
            cache: Mutex::new(HashMap::new()),
            last_conflicts: Mutex::new(HashMap::new()),
        })
    }

    pub fn enrich_event(&self, ev: &mut Event) -> IdentityEnrichment {
        if !self.enable {
            return IdentityEnrichment::default();
        }
        let Some(task_id) = ev.host_tid.or(ev.host_pid) else {
            return IdentityEnrichment::default();
        };
        let resolution = self.resolve_pid_with_metrics(task_id);
        let ident = resolution.identity;
        ev.host_start_time_ticks = ident.host_start_time_ticks;
        ev.vm_id = Some(ident.vm_id.clone());
        merge_event_identity(ev, &ident);
        if let Some(name) = ident.vm_name.clone() {
            ev.vm_name = Some(name.clone());
            ev.vm = name;
        }
        if ev.vcpu_id.is_none() {
            if let Some(vcpu_id) = ident.vcpu_id {
                ev.vcpu_id = Some(vcpu_id);
                ev.vcpu = Some(vcpu_id);
                push_tag(&mut ev.tags, "identity:vcpu-map");
            } else if ident.vcpu_ambiguous {
                push_tag(&mut ev.tags, "identity:vcpu-ambiguous");
            }
        }
        push_tag(&mut ev.tags, "identity:proc");
        if ident.libvirt_uuid.is_some() {
            push_tag(&mut ev.tags, "identity:libvirt");
        }
        if ident.ambiguous {
            push_tag(&mut ev.tags, "identity:ambiguous");
        }
        if let Some(conflict) = &ident.identity_conflict {
            push_tag(&mut ev.tags, "identity:conflict");
            push_tag(
                &mut ev.tags,
                &format!("identity_conflict:{}", conflict.reason.as_str()),
            );
        }
        if ident.qmp_socket.is_some() {
            push_tag(&mut ev.tags, "identity:qmp-hint");
        }
        let conflict_reason = ident
            .identity_conflict
            .as_ref()
            .map(|conflict| conflict.reason);
        IdentityEnrichment {
            conflict_event: self.identity_conflict_event(&ident),
            cache_result: Some(resolution.cache_result),
            confidence: Some(ident.identity_confidence),
            ambiguous: ident.ambiguous,
            conflict_reason,
        }
    }

    pub fn resolve_pid(&self, pid: i32) -> VmIdentity {
        self.resolve_pid_with_metrics(pid).identity
    }

    pub fn resolve_pid_with_metrics(&self, pid: i32) -> IdentityResolution {
        let now = Instant::now();
        let start_time = if self.live_host_lookup {
            read_proc_start_time_ticks(pid)
        } else {
            None
        };
        let mut stale_cache_conflict = None;
        let mut cache_result = IdentityCacheResult::Miss;
        if let Ok(cache) = self.cache.lock() {
            if let Some(cached) = cache.get(&pid) {
                if cache_entry_matches(cached, start_time, now, self.ttl) {
                    return IdentityResolution {
                        identity: cached.value.clone(),
                        cache_result: IdentityCacheResult::Hit,
                    };
                }
                if matches!(
                    (cached.start_time_ticks, start_time),
                    (Some(expected), Some(observed)) if expected != observed
                ) {
                    cache_result = IdentityCacheResult::Refusal;
                    stale_cache_conflict = Some(IdentityConflict {
                        task_id: pid,
                        reason: IdentityConflictReason::StaleCache,
                        ambiguous: false,
                    });
                }
            }
        }
        let mut ident = if self.live_host_lookup {
            resolve_identity_once_with_start_time(pid, &self.qmp_socket_dirs, start_time)
        } else {
            fallback_pid_identity(pid)
        };
        if self.live_host_lookup {
            if let Ok(libvirt) = self.libvirt.lock() {
                if let Some(libvirt) = libvirt.as_ref() {
                    apply_libvirt_discovery(pid, &mut ident, libvirt);
                }
            }
        }
        if ident.vm_id.is_empty() {
            ident.vm_id = stable_pid_id(pid, ident.host_start_time_ticks);
        }
        if ident.identity_conflict.is_none() {
            ident.identity_conflict = stale_cache_conflict;
        }
        if ident.host_start_time_ticks.is_some() && !ident.ambiguous {
            if let Ok(mut cache) = self.cache.lock() {
                cache.insert(
                    pid,
                    CachedIdentity {
                        seen_at: now,
                        start_time_ticks: ident.host_start_time_ticks,
                        value: ident.clone(),
                    },
                );
            }
        }
        IdentityResolution {
            identity: ident,
            cache_result,
        }
    }

    pub fn inventory_snapshot(&self) -> VmInventorySnapshot {
        if !self.enable {
            return VmInventorySnapshot::disabled();
        }
        match self.libvirt.lock() {
            Ok(libvirt) => libvirt
                .as_ref()
                .map(LibvirtDomainDiscovery::inventory_snapshot)
                .unwrap_or_else(VmInventorySnapshot::empty),
            Err(_) => VmInventorySnapshot::degraded_empty(),
        }
    }

    fn identity_conflict_event(&self, ident: &VmIdentity) -> Option<Event> {
        let conflict = ident.identity_conflict.as_ref()?;
        let key = IdentityConflictKey {
            task_id: conflict.task_id,
            reason: conflict.reason,
        };
        let now = Instant::now();
        if let Ok(mut last_conflicts) = self.last_conflicts.lock() {
            if let Some(previous) = last_conflicts.get(&key) {
                if now.saturating_duration_since(*previous) < IDENTITY_CONFLICT_COOLDOWN {
                    return None;
                }
            }
            last_conflicts.insert(key, now);
        }
        Some(identity_conflict_sensor_event(ident, conflict))
    }

    #[cfg(test)]
    fn cache_len(&self) -> usize {
        self.cache.lock().map(|cache| cache.len()).unwrap_or(0)
    }
}

fn cache_entry_matches(
    cached: &CachedIdentity,
    observed_start_time_ticks: Option<u64>,
    now: Instant,
    ttl: Duration,
) -> bool {
    matches!(
        (cached.start_time_ticks, observed_start_time_ticks),
        (Some(expected), Some(observed)) if expected == observed
    ) && now.saturating_duration_since(cached.seen_at) <= ttl
}

impl VmIdentityResolver {
    pub fn drain_libvirt_lifecycle<S>(
        &self,
        source: &mut S,
    ) -> Result<Vec<Event>, LibvirtLifecycleError>
    where
        S: LibvirtLifecycleSource,
    {
        let mut events = Vec::new();
        while let Some(event) = source.next_event()? {
            events.push(self.apply_libvirt_lifecycle_event(event)?);
        }
        Ok(events)
    }

    pub fn apply_libvirt_lifecycle_event(
        &self,
        event: LibvirtLifecycleEvent,
    ) -> Result<Event, LibvirtLifecycleError> {
        let uuid = event.uuid.trim();
        if uuid.is_empty() {
            return Err(LibvirtLifecycleError::MalformedEvent(
                "missing domain uuid".to_string(),
            ));
        }

        let tasks = event.task_ids();
        let mut discovery_action = "unchanged";
        match event.kind {
            LibvirtLifecycleKind::Started
            | LibvirtLifecycleKind::Resumed
            | LibvirtLifecycleKind::Migrated => {
                if let Some(domain) = event.to_domain() {
                    self.upsert_libvirt_domain(domain);
                    discovery_action = "updated";
                }
            }
            LibvirtLifecycleKind::Stopped => {
                if self.remove_libvirt_domain(uuid) {
                    discovery_action = "removed";
                }
            }
            LibvirtLifecycleKind::Paused => {}
        }
        let invalidated = self.invalidate_libvirt_cache(uuid, &tasks);
        Ok(libvirt_lifecycle_sensor_event(
            &event,
            invalidated,
            discovery_action,
        ))
    }

    fn upsert_libvirt_domain(&self, domain: LibvirtDomain) {
        if let Ok(mut libvirt) = self.libvirt.lock() {
            match libvirt.as_mut() {
                Some(discovery) => discovery.upsert_domain(domain),
                None => *libvirt = Some(LibvirtDomainDiscovery::from_domains(vec![domain])),
            }
        }
    }

    fn remove_libvirt_domain(&self, uuid: &str) -> bool {
        self.libvirt
            .lock()
            .ok()
            .and_then(|mut libvirt| {
                libvirt
                    .as_mut()
                    .map(|discovery| discovery.remove_domain_uuid(uuid))
            })
            .unwrap_or(false)
    }

    fn invalidate_libvirt_cache(&self, uuid: &str, tasks: &[i32]) -> usize {
        let Ok(mut cache) = self.cache.lock() else {
            return 0;
        };
        let before = cache.len();
        cache.retain(|task_id, cached| {
            let task_match = tasks.binary_search(task_id).is_ok();
            let uuid_match = cached.value.libvirt_uuid.as_deref() == Some(uuid);
            !(task_match || uuid_match)
        });
        before.saturating_sub(cache.len())
    }
}

fn libvirt_lifecycle_sensor_event(
    event: &LibvirtLifecycleEvent,
    invalidated: usize,
    discovery_action: &str,
) -> Event {
    let mut ev = Event::base(
        Category::Sensor,
        Severity::Info,
        now_rfc3339(),
        "host".to_string(),
    );
    ev.reason = Some("libvirt_lifecycle".to_string());
    ev.vm_id = Some(format!("libvirt:{}", event.uuid.trim()));
    ev.vm_name = event.name.clone();
    ev.tags = vec![
        "identity:libvirt".to_string(),
        "identity:lifecycle".to_string(),
    ];
    ev.identity = Some(IdentityInfo {
        sources: vec![IDENTITY_SOURCE_LIBVIRT_LIFECYCLE.to_string()],
        confidence: IdentityConfidence::Medium,
        start_time_verified: false,
        ambiguous: false,
    });
    ev.message = Some(format!(
        "libvirt lifecycle kind={} discovery={} cache_invalidated={} task_count={}; file-backed XML discovery remains compatible and live daemon subscription is not enabled by default",
        event.kind.as_str(),
        discovery_action,
        invalidated,
        event.task_ids().len()
    ));
    ev
}

fn identity_conflict_sensor_event(ident: &VmIdentity, conflict: &IdentityConflict) -> Event {
    let mut ev = Event::base(
        Category::Sensor,
        conflict.reason.severity(),
        now_rfc3339(),
        "host".to_string(),
    );
    ev.reason = Some("identity_conflict".to_string());
    ev.vm_id = if ident.vm_id.is_empty() {
        Some(stable_pid_id(conflict.task_id, ident.host_start_time_ticks))
    } else {
        Some(ident.vm_id.clone())
    };
    if ident.host_pid == conflict.task_id {
        ev.host_pid = Some(conflict.task_id);
    } else {
        ev.host_tid = Some(conflict.task_id);
    }
    ev.host_start_time_ticks = ident.host_start_time_ticks;
    ev.identity = Some(IdentityInfo {
        sources: ident.identity_sources.clone(),
        confidence: ident.identity_confidence,
        start_time_verified: ident.start_time_verified,
        ambiguous: ident.ambiguous || conflict.ambiguous,
    });
    ev.tags = vec![
        "identity:conflict".to_string(),
        format!("identity_conflict:{}", conflict.reason.as_str()),
    ];
    let outcome = if conflict.ambiguous {
        "identity_degraded"
    } else {
        "diagnostic_only"
    };
    ev.message = Some(format!(
        "identity conflict reason={} task_id={} outcome={}",
        conflict.reason.as_str(),
        conflict.task_id,
        outcome
    ));
    ev
}

fn inventory_vm_json(vm: &VmInventoryVm) -> String {
    let host_tasks = vm
        .known_host_tasks
        .iter()
        .map(inventory_host_task_json)
        .collect::<Vec<_>>()
        .join(",");
    let vcpu_mappings = vm
        .vcpu_mappings
        .iter()
        .map(inventory_vcpu_mapping_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"vm_id\":{},\"vm_uuid\":{},\"vm_name\":{},\"source\":{},\"known_host_tasks\":[{}],\"vcpu_mappings\":[{}],\"qmp\":{},\"identity\":{},\"ambiguous\":{},\"conflict\":{}}}",
        json_str(&vm.vm_id),
        json_str(&vm.vm_uuid),
        json_str(&vm.vm_name),
        json_str(&vm.source),
        host_tasks,
        vcpu_mappings,
        inventory_qmp_json(&vm.qmp),
        inventory_identity_json(&vm.identity),
        vm.ambiguous,
        inventory_conflict_json(&vm.conflict)
    )
}

fn inventory_host_task_json(task: &VmInventoryHostTask) -> String {
    format!(
        "{{\"kind\":{},\"id\":{},\"start_time_ticks\":{}}}",
        json_str(&task.kind),
        task.id,
        json_opt_u64(task.start_time_ticks)
    )
}

fn inventory_vcpu_mapping_json(mapping: &VmInventoryVcpuMapping) -> String {
    format!(
        "{{\"host_tid\":{},\"vcpu_id\":{},\"start_time_ticks\":{}}}",
        mapping.host_tid,
        mapping.vcpu_id,
        json_opt_u64(mapping.start_time_ticks)
    )
}

fn inventory_qmp_json(qmp: &VmInventoryQmp) -> String {
    format!(
        "{{\"present\":{},\"status\":{}}}",
        qmp.present,
        json_str(&qmp.status)
    )
}

fn inventory_identity_json(identity: &IdentityInfo) -> String {
    let sources = identity
        .sources
        .iter()
        .map(|source| json_str(source))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"sources\":[{}],\"confidence\":{},\"start_time_verified\":{},\"ambiguous\":{}}}",
        sources,
        json_str(identity.confidence.as_str()),
        identity.start_time_verified,
        identity.ambiguous
    )
}

fn inventory_conflict_json(conflict: &Option<VmInventoryConflict>) -> String {
    match conflict {
        Some(conflict) => format!(
            "{{\"reason\":{},\"ambiguous\":{}}}",
            json_str(&conflict.reason),
            conflict.ambiguous
        ),
        None => "null".to_string(),
    }
}

fn json_opt_u64(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn push_tag(tags: &mut Vec<String>, tag: &str) {
    if tags.iter().all(|t| t != tag) {
        tags.push(tag.to_string());
    }
}

fn merge_event_identity(ev: &mut Event, ident: &VmIdentity) {
    let mut identity = ev.identity.clone().unwrap_or(IdentityInfo {
        sources: Vec::new(),
        confidence: IdentityConfidence::Low,
        start_time_verified: false,
        ambiguous: false,
    });
    for source in &ident.identity_sources {
        push_identity_source(&mut identity.sources, source);
    }
    identity.start_time_verified |= ident.start_time_verified;
    identity.ambiguous |= ident.ambiguous;
    identity.confidence = if identity.ambiguous {
        IdentityConfidence::Low
    } else {
        identity.confidence.max(ident.identity_confidence)
    };
    ev.identity = Some(identity);
}

fn push_identity_source(sources: &mut Vec<String>, source: &str) {
    if sources.iter().all(|candidate| candidate != source) {
        sources.push(source.to_string());
    }
}

fn identity_has_source(ident: &VmIdentity, source: &str) -> bool {
    ident
        .identity_sources
        .iter()
        .any(|candidate| candidate == source)
}

fn recompute_identity_confidence(ident: &mut VmIdentity) {
    ident.identity_confidence = if ident.ambiguous {
        IdentityConfidence::Low
    } else if ident.start_time_verified
        && ident.libvirt_uuid.is_some()
        && (identity_has_source(ident, IDENTITY_SOURCE_LIBVIRT_XML)
            || identity_has_source(ident, IDENTITY_SOURCE_LIBVIRT_LIFECYCLE))
    {
        IdentityConfidence::High
    } else if ident.libvirt_uuid.is_some()
        && (identity_has_source(ident, IDENTITY_SOURCE_LIBVIRT_XML)
            || identity_has_source(ident, IDENTITY_SOURCE_LIBVIRT_LIFECYCLE)
            || identity_has_source(ident, IDENTITY_SOURCE_PROC_CMDLINE))
    {
        IdentityConfidence::Medium
    } else {
        IdentityConfidence::Low
    };
}

pub fn resolve_identity_once(pid: i32, qmp_socket_dirs: &[PathBuf]) -> VmIdentity {
    let start_time = read_proc_start_time_ticks(pid);
    resolve_identity_once_with_start_time(pid, qmp_socket_dirs, start_time)
}

fn fallback_pid_identity(pid: i32) -> VmIdentity {
    let mut ident = VmIdentity {
        host_pid: pid,
        vm_id: stable_pid_id(pid, None),
        ..VmIdentity::default()
    };
    push_identity_source(&mut ident.identity_sources, IDENTITY_SOURCE_FALLBACK_PID);
    recompute_identity_confidence(&mut ident);
    ident
}

fn resolve_identity_once_with_start_time(
    pid: i32,
    qmp_socket_dirs: &[PathBuf],
    start_time: Option<u64>,
) -> VmIdentity {
    let mut ident = VmIdentity {
        host_pid: pid,
        host_start_time_ticks: start_time,
        vm_id: stable_pid_id(pid, start_time),
        ..VmIdentity::default()
    };
    if let Some(args) = read_cmdline(pid) {
        let vm_name = parse_qemu_name(&args).or_else(|| parse_arg_value(&args, "-name"));
        let libvirt_uuid = parse_arg_value(&args, "-uuid");
        let qmp_socket = parse_monitor_socket(&args);
        if vm_name.is_some() || libvirt_uuid.is_some() || qmp_socket.is_some() {
            push_identity_source(&mut ident.identity_sources, IDENTITY_SOURCE_PROC_CMDLINE);
        }
        ident.vm_name = vm_name;
        ident.libvirt_uuid = libvirt_uuid;
        if qmp_socket.is_some() {
            push_identity_source(&mut ident.identity_sources, IDENTITY_SOURCE_QMP_SOCKET_HINT);
        }
        ident.qmp_socket = qmp_socket;
    }
    if let Some(cg) = read_cgroup(pid) {
        let cgroup_name = parse_cgroup_domain_name(&cg);
        let cgroup_unit = parse_cgroup_unit(&cg);
        merge_cgroup_identity(&mut ident, pid, cgroup_name, cgroup_unit);
    }
    if !ident.ambiguous {
        if let Some(uuid) = &ident.libvirt_uuid {
            ident.vm_id = format!("libvirt:{uuid}");
        } else if let Some(name) = &ident.vm_name {
            ident.vm_id = format!("name:{name}");
        }
        if ident.qmp_socket.is_none() {
            if let Some(name) = &ident.vm_name {
                ident.qmp_socket = discover_qmp_socket(name, qmp_socket_dirs);
                if ident.qmp_socket.is_some() {
                    push_identity_source(
                        &mut ident.identity_sources,
                        IDENTITY_SOURCE_QMP_SOCKET_HINT,
                    );
                }
            }
        }
    }
    if ident.identity_sources.is_empty() {
        push_identity_source(&mut ident.identity_sources, IDENTITY_SOURCE_FALLBACK_PID);
    }
    recompute_identity_confidence(&mut ident);
    ident
}

fn merge_cgroup_identity(
    ident: &mut VmIdentity,
    task_id: i32,
    cgroup_name: Option<String>,
    cgroup_unit: Option<String>,
) {
    let has_cgroup_identity = cgroup_name.is_some() || cgroup_unit.is_some();
    if has_cgroup_identity {
        push_identity_source(&mut ident.identity_sources, IDENTITY_SOURCE_PROC_CGROUP);
    }
    if let Some(cgroup_name) = cgroup_name {
        match ident.vm_name.as_ref() {
            Some(existing) if existing != &cgroup_name && ident.libvirt_uuid.is_none() => {
                mark_ambiguous_host_task(
                    ident,
                    task_id,
                    IdentityConflictReason::ProcCgroupMismatch,
                );
            }
            Some(existing) if existing != &cgroup_name => {
                record_identity_conflict(
                    ident,
                    task_id,
                    IdentityConflictReason::ProcCgroupMismatch,
                    false,
                );
            }
            Some(_) => {}
            None => ident.vm_name = Some(cgroup_name),
        }
    }
    ident.cgroup_unit = cgroup_unit;
}

fn apply_libvirt_discovery(
    task_id: i32,
    ident: &mut VmIdentity,
    discovery: &LibvirtDomainDiscovery,
) {
    match discovery.lookup_task_with_start_time(task_id, ident.host_start_time_ticks) {
        Ok(Some(domain)) => {
            if ident
                .libvirt_uuid
                .as_ref()
                .is_some_and(|uuid| uuid != &domain.uuid)
            {
                mark_ambiguous_host_task(
                    ident,
                    task_id,
                    IdentityConflictReason::LibvirtUuidMismatch,
                );
                return;
            }
            if ident
                .qmp_socket
                .as_ref()
                .zip(domain.qmp_socket.as_ref())
                .is_some_and(|(current, mapped)| current != mapped)
            {
                mark_ambiguous_host_task(ident, task_id, IdentityConflictReason::QmpSocketMismatch);
                return;
            }
            let libvirt_name_mismatch = ident
                .vm_name
                .as_ref()
                .is_some_and(|name| name != &domain.name);
            let start_time_verified =
                domain_task_start_time_verified(domain, task_id, ident.host_start_time_ticks);
            ident.vm_id = format!("libvirt:{}", domain.uuid);
            ident.vm_name = Some(domain.name.clone());
            ident.libvirt_uuid = Some(domain.uuid.clone());
            push_identity_source(&mut ident.identity_sources, domain.source.identity_source());
            if let Some(socket) = &domain.qmp_socket {
                ident.qmp_socket = Some(socket.clone());
                push_identity_source(&mut ident.identity_sources, IDENTITY_SOURCE_QMP_SOCKET_HINT);
            }
            if start_time_verified {
                ident.start_time_verified = true;
                push_identity_source(
                    &mut ident.identity_sources,
                    IDENTITY_SOURCE_START_TIME_VERIFIED,
                );
            }
            match lookup_vcpu_thread(domain, task_id) {
                VcpuThreadLookup::Known(vcpu_id) => {
                    ident.vcpu_id = Some(vcpu_id);
                    ident.vcpu_ambiguous = false;
                }
                VcpuThreadLookup::Unknown => {}
                VcpuThreadLookup::Ambiguous => {
                    ident.vcpu_id = None;
                    ident.vcpu_ambiguous = true;
                }
            }
            ident.ambiguous = false;
            recompute_identity_confidence(ident);
            if libvirt_name_mismatch {
                record_identity_conflict(
                    ident,
                    task_id,
                    IdentityConflictReason::LibvirtNameMismatch,
                    false,
                );
            }
        }
        Ok(None) => {}
        Err(LibvirtDiscoveryError::AmbiguousTask { .. }) => {
            mark_ambiguous_host_task(ident, task_id, IdentityConflictReason::MultipleDomains);
        }
        Err(LibvirtDiscoveryError::PidReuseDetected { .. }) => {
            mark_ambiguous_host_task(ident, task_id, IdentityConflictReason::PidReuse);
        }
        Err(LibvirtDiscoveryError::UnverifiedTaskIdentity { .. }) => {
            mark_ambiguous_host_task(ident, task_id, IdentityConflictReason::StartTimeUnverified);
        }
        Err(_) => {}
    }
}

fn mark_ambiguous_host_task(ident: &mut VmIdentity, task_id: i32, reason: IdentityConflictReason) {
    ident.vm_id = match reason.vm_id_suffix() {
        Some(suffix) => format!("ambiguous:host-task:{task_id}:{suffix}"),
        None => format!("ambiguous:host-task:{task_id}"),
    };
    ident.vm_name = None;
    ident.libvirt_uuid = None;
    ident.qmp_socket = None;
    ident.vcpu_id = None;
    ident.vcpu_ambiguous = false;
    ident.ambiguous = true;
    ident.identity_sources.clear();
    push_identity_source(&mut ident.identity_sources, IDENTITY_SOURCE_AMBIGUOUS);
    ident.identity_confidence = IdentityConfidence::Low;
    ident.start_time_verified = false;
    ident.identity_conflict = Some(IdentityConflict {
        task_id,
        reason,
        ambiguous: true,
    });
}

fn record_identity_conflict(
    ident: &mut VmIdentity,
    task_id: i32,
    reason: IdentityConflictReason,
    ambiguous: bool,
) {
    if ambiguous {
        mark_ambiguous_host_task(ident, task_id, reason);
    } else {
        ident.identity_conflict = Some(IdentityConflict {
            task_id,
            reason,
            ambiguous: false,
        });
    }
}

fn domain_task_start_time_verified(
    domain: &LibvirtDomain,
    task_id: i32,
    observed_start_time_ticks: Option<u64>,
) -> bool {
    let Some(observed) = observed_start_time_ticks else {
        return false;
    };
    domain
        .qemu_task_identities
        .iter()
        .any(|identity| identity.task_id == task_id && identity.start_time_ticks == Some(observed))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VcpuThreadLookup {
    Known(i32),
    Unknown,
    Ambiguous,
}

fn lookup_vcpu_thread(domain: &LibvirtDomain, tid: i32) -> VcpuThreadLookup {
    let mut found: Option<i32> = None;
    for mapping in domain
        .vcpu_threads
        .iter()
        .filter(|mapping| mapping.tid == tid)
    {
        match found {
            Some(existing) if existing != mapping.vcpu_id => return VcpuThreadLookup::Ambiguous,
            Some(_) => {}
            None => found = Some(mapping.vcpu_id),
        }
    }
    found.map_or(VcpuThreadLookup::Unknown, VcpuThreadLookup::Known)
}

pub fn parse_libvirt_domain_xml(
    path: &Path,
    xml: &str,
) -> Result<LibvirtDomain, LibvirtDiscoveryError> {
    let name = extract_xml_text(xml, "name")
        .ok_or_else(|| libvirt_malformed(path, "missing required <name> element in domain XML"))?;
    let uuid = extract_xml_text(xml, "uuid")
        .ok_or_else(|| libvirt_malformed(path, "missing required <uuid> element in domain XML"))?;
    let qemu_pids = extract_i32_attrs(xml, "pid");
    let qemu_tids = extract_i32_attrs(xml, "tid");
    if qemu_pids.is_empty() && qemu_tids.is_empty() {
        return Err(libvirt_malformed(
            path,
            "missing mocked qemu pid or tid attribute for AegisHV discovery",
        ));
    }
    let qmp_socket = extract_attr(xml, "qmp_socket");
    let vcpu_threads = extract_vcpu_thread_mappings(path, xml)?;
    let qemu_task_identities = extract_host_task_identities(path, xml)?;

    Ok(LibvirtDomain {
        name,
        uuid,
        qmp_socket,
        qemu_pids,
        qemu_tids,
        vcpu_threads,
        qemu_task_identities,
        source: LibvirtDomainSource::Xml,
    })
}

fn libvirt_malformed(path: &Path, detail: impl Into<String>) -> LibvirtDiscoveryError {
    LibvirtDiscoveryError::MalformedDomain {
        path: path.to_path_buf(),
        detail: detail.into(),
    }
}

fn extract_vcpu_thread_mappings(
    path: &Path,
    xml: &str,
) -> Result<Vec<QemuVcpuThread>, LibvirtDiscoveryError> {
    let mut mappings = Vec::new();
    for fragment in xml.split('<').skip(1) {
        let tag = fragment.split('>').next().unwrap_or("");
        let tid = match extract_attr(tag, "tid") {
            Some(value) => Some(value.parse::<i32>().map_err(|_| {
                libvirt_malformed(path, format!("invalid vCPU thread tid '{value}'"))
            })?),
            None => None,
        };
        let vcpu = match extract_attr(tag, "vcpu_id") {
            Some(value) => Some(value.parse::<i32>().map_err(|_| {
                libvirt_malformed(path, format!("invalid vcpu_id '{value}' for QEMU thread"))
            })?),
            None => None,
        };
        match (tid, vcpu) {
            (Some(tid), Some(vcpu_id)) if tid > 0 && vcpu_id >= 0 => {
                insert_vcpu_thread_mapping(path, &mut mappings, QemuVcpuThread { tid, vcpu_id })?;
            }
            (Some(_), Some(_)) => {
                return Err(libvirt_malformed(
                    path,
                    "vCPU thread metadata requires tid > 0 and vcpu_id >= 0",
                ));
            }
            (None, Some(_)) => {
                return Err(libvirt_malformed(
                    path,
                    "vCPU thread metadata has vcpu_id without tid",
                ));
            }
            _ => {}
        }
    }
    Ok(mappings)
}

fn extract_host_task_identities(
    path: &Path,
    xml: &str,
) -> Result<Vec<HostTaskIdentity>, LibvirtDiscoveryError> {
    let mut identities = Vec::new();
    for fragment in xml.split('<').skip(1) {
        let tag = fragment.split('>').next().unwrap_or("");
        if let Some(pid) = parse_task_id_attr(path, tag, "pid")? {
            let start_time_ticks = match parse_start_time_attr(path, tag, "pid_start_time_ticks")? {
                Some(ticks) => Some(ticks),
                None => parse_start_time_attr(path, tag, "start_time_ticks")?,
            };
            insert_host_task_identity(
                path,
                &mut identities,
                HostTaskIdentity {
                    task_id: pid,
                    start_time_ticks,
                },
            )?;
        }
        if let Some(tid) = parse_task_id_attr(path, tag, "tid")? {
            let start_time_ticks = match parse_start_time_attr(path, tag, "tid_start_time_ticks")? {
                Some(ticks) => Some(ticks),
                None => parse_start_time_attr(path, tag, "start_time_ticks")?,
            };
            insert_host_task_identity(
                path,
                &mut identities,
                HostTaskIdentity {
                    task_id: tid,
                    start_time_ticks,
                },
            )?;
        }
    }
    Ok(identities)
}

fn parse_task_id_attr(
    path: &Path,
    tag: &str,
    attr: &str,
) -> Result<Option<i32>, LibvirtDiscoveryError> {
    let Some(value) = extract_attr(tag, attr) else {
        return Ok(None);
    };
    let task_id = value
        .parse::<i32>()
        .map_err(|_| libvirt_malformed(path, format!("invalid host task {attr} '{value}'")))?;
    if task_id <= 0 {
        return Err(libvirt_malformed(
            path,
            format!("host task {attr} must be greater than zero"),
        ));
    }
    Ok(Some(task_id))
}

fn parse_start_time_attr(
    path: &Path,
    tag: &str,
    attr: &str,
) -> Result<Option<u64>, LibvirtDiscoveryError> {
    let Some(value) = extract_attr(tag, attr) else {
        return Ok(None);
    };
    let ticks = value.parse::<u64>().map_err(|_| {
        libvirt_malformed(
            path,
            format!("invalid process start_time_ticks attribute {attr}='{value}'"),
        )
    })?;
    Ok(Some(ticks))
}

fn insert_host_task_identity(
    path: &Path,
    identities: &mut Vec<HostTaskIdentity>,
    next: HostTaskIdentity,
) -> Result<(), LibvirtDiscoveryError> {
    if let Some(existing) = identities
        .iter_mut()
        .find(|identity| identity.task_id == next.task_id)
    {
        match (existing.start_time_ticks, next.start_time_ticks) {
            (Some(current), Some(candidate)) if current != candidate => {
                return Err(libvirt_malformed(
                    path,
                    format!(
                        "conflicting process start_time_ticks for host task {}: {} and {}",
                        next.task_id, current, candidate
                    ),
                ));
            }
            (None, Some(candidate)) => existing.start_time_ticks = Some(candidate),
            _ => {}
        }
        return Ok(());
    }
    identities.push(next);
    Ok(())
}

fn insert_vcpu_thread_mapping(
    path: &Path,
    mappings: &mut Vec<QemuVcpuThread>,
    next: QemuVcpuThread,
) -> Result<(), LibvirtDiscoveryError> {
    if let Some(existing) = mappings.iter().find(|mapping| mapping.tid == next.tid) {
        if existing.vcpu_id == next.vcpu_id {
            return Ok(());
        }
        return Err(libvirt_malformed(
            path,
            format!(
                "conflicting vCPU metadata for host tid {}: {} and {}",
                next.tid, existing.vcpu_id, next.vcpu_id
            ),
        ));
    }
    mappings.push(next);
    Ok(())
}

fn extract_xml_text(xml: &str, tag: &str) -> Option<String> {
    let start = format!("<{tag}>");
    let end = format!("</{tag}>");
    let value = xml.split(&start).nth(1)?.split(&end).next()?.trim();
    if value.is_empty() {
        None
    } else {
        Some(xml_unescape(value))
    }
}

fn extract_i32_attrs(xml: &str, attr: &str) -> Vec<i32> {
    extract_attrs(xml, attr)
        .into_iter()
        .filter_map(|value| value.parse::<i32>().ok())
        .collect()
}

fn extract_attr(xml: &str, attr: &str) -> Option<String> {
    extract_attrs(xml, attr)
        .into_iter()
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

fn extract_attrs(xml: &str, attr: &str) -> Vec<String> {
    let mut values = Vec::new();
    let bytes = xml.as_bytes();
    let mut index = 0usize;
    while let Some(pos) = xml[index..].find(attr) {
        let attr_start = index + pos;
        let before = attr_start.checked_sub(1).and_then(|idx| bytes.get(idx));
        let after = bytes.get(attr_start + attr.len());
        let valid_before = before.map_or(true, |b| !is_attr_name_byte(*b));
        let valid_after = after == Some(&b'=');
        if !valid_before || !valid_after {
            index = attr_start + attr.len();
            continue;
        }
        let quote_pos = attr_start + attr.len() + 1;
        let Some(&quote) = bytes.get(quote_pos) else {
            break;
        };
        if quote != b'"' && quote != b'\'' {
            index = attr_start + attr.len();
            continue;
        }
        let value_start = quote_pos + 1;
        let Some(value_end_rel) = bytes[value_start..].iter().position(|b| *b == quote) else {
            break;
        };
        let value_end = value_start + value_end_rel;
        if let Some(value) = xml.get(value_start..value_end) {
            values.push(xml_unescape(value));
        }
        index = value_end + 1;
    }
    values
}

fn is_attr_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b':')
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn stable_pid_id(pid: i32, start: Option<u64>) -> String {
    match start {
        Some(ticks) => format!("host-pid:{pid}:start:{ticks}"),
        None => format!("host-pid:{pid}"),
    }
}

pub fn read_proc_start_time_ticks(pid: i32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parse_proc_stat_start_time(&stat)
}

pub fn parse_proc_stat_start_time(stat: &str) -> Option<u64> {
    // /proc/<pid>/stat has comm in parentheses and may contain spaces. starttime is field 22.
    let rparen = stat.rfind(')')?;
    let rest = stat.get(rparen + 2..)?;
    let fields: Vec<&str> = rest.split_whitespace().collect();
    fields.get(19)?.parse::<u64>().ok()
}

fn read_cmdline(pid: i32) -> Option<Vec<String>> {
    let data = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let args = data
        .split(|b| *b == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).to_string())
        .collect::<Vec<_>>();
    if args.is_empty() {
        None
    } else {
        Some(args)
    }
}

fn read_cgroup(pid: i32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()
}

pub fn parse_arg_value(args: &[String], flag: &str) -> Option<String> {
    for (idx, arg) in args.iter().enumerate() {
        if arg == flag {
            return args
                .get(idx + 1)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        }
        if let Some(v) = arg.strip_prefix(&format!("{flag}=")) {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

pub fn parse_qemu_name(args: &[String]) -> Option<String> {
    let raw = parse_arg_value(args, "-name")?;
    for part in raw.split(',') {
        if let Some(v) = part.strip_prefix("guest=") {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    raw.split(',')
        .next()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn parse_monitor_socket(args: &[String]) -> Option<String> {
    for (idx, arg) in args.iter().enumerate() {
        if arg == "-qmp" || arg == "-monitor" {
            if let Some(v) = args.get(idx + 1) {
                if let Some(sock) = parse_unix_socket_arg(v) {
                    return Some(sock);
                }
            }
        } else if let Some(v) = arg
            .strip_prefix("-qmp=")
            .or_else(|| arg.strip_prefix("-monitor="))
        {
            if let Some(sock) = parse_unix_socket_arg(v) {
                return Some(sock);
            }
        }
    }
    None
}

fn parse_unix_socket_arg(v: &str) -> Option<String> {
    let value = v.strip_prefix("unix:").unwrap_or(v);
    let path = value.split(',').next()?.trim();
    if path.starts_with('/') {
        Some(path.to_string())
    } else {
        None
    }
}

pub fn parse_cgroup_domain_name(cgroup: &str) -> Option<String> {
    for line in cgroup.lines() {
        let path = line.rsplit(':').next().unwrap_or(line);
        if let Some(name) = path.split("/libvirt/qemu/").nth(1) {
            return Some(name.trim_matches('/').to_string()).filter(|s| !s.is_empty());
        }
        if let Some(unit) = path
            .split('/')
            .find(|p| p.contains("qemu") && p.ends_with(".scope"))
        {
            let decoded = decode_systemd_escape(unit.trim_end_matches(".scope"));
            let decoded = decoded
                .trim_start_matches("machine-qemu-")
                .trim_start_matches("qemu-")
                .to_string();
            if !decoded.is_empty() {
                return Some(decoded);
            }
        }
    }
    None
}

fn parse_cgroup_unit(cgroup: &str) -> Option<String> {
    for line in cgroup.lines() {
        let path = line.rsplit(':').next().unwrap_or(line);
        if let Some(unit) = path
            .split('/')
            .find(|p| p.ends_with(".scope") || p.ends_with(".service"))
        {
            return Some(decode_systemd_escape(unit));
        }
    }
    None
}

fn decode_systemd_escape(s: &str) -> String {
    let mut out = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 3 < bytes.len() && bytes[i + 1] == b'x' {
            let h = &s[i + 2..i + 4];
            if let Ok(v) = u8::from_str_radix(h, 16) {
                out.push(v as char);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn discover_qmp_socket(vm_name: &str, dirs: &[PathBuf]) -> Option<String> {
    for dir in dirs {
        let candidates = [
            dir.join(format!("{vm_name}.monitor")),
            dir.join(format!("{vm_name}.qmp")),
            dir.join(vm_name).join("monitor.sock"),
            dir.join(vm_name).join("qmp.sock"),
        ];
        for p in candidates {
            if is_socketish(&p) {
                return Some(p.display().to_string());
            }
        }
    }
    None
}

fn is_socketish(p: &Path) -> bool {
    if !p.exists() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        if let Ok(meta) = std::fs::symlink_metadata(p) {
            return meta.file_type().is_socket();
        }
    }
    false
}

pub fn parse_vcpu_id_from_thread_name(name: &str) -> Option<i32> {
    let lower = name.to_ascii_lowercase();
    if let Some(pos) = lower.find("vcpu") {
        let digits: String = lower[pos + 4..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if !digits.is_empty() {
            return digits.parse().ok();
        }
    }
    if let Some(pos) = lower.find("cpu") {
        let tail = &lower[pos + 3..];
        let digits: String = tail
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if !digits.is_empty() {
            return digits.parse().ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_qemu_name_and_uuid() {
        let args = vec![
            "qemu-system-x86_64".to_string(),
            "-name".to_string(),
            "guest=win11,debug-threads=on".to_string(),
            "-uuid".to_string(),
            "11111111-2222-3333-4444-555555555555".to_string(),
        ];
        assert_eq!(parse_qemu_name(&args).as_deref(), Some("win11"));
        assert_eq!(
            parse_arg_value(&args, "-uuid").as_deref(),
            Some("11111111-2222-3333-4444-555555555555")
        );
    }

    #[test]
    fn parses_vcpu_thread_names() {
        assert_eq!(parse_vcpu_id_from_thread_name("CPU 7/KVM"), Some(7));
        assert_eq!(parse_vcpu_id_from_thread_name("vcpu2"), Some(2));
        assert_eq!(parse_vcpu_id_from_thread_name("worker"), None);
    }

    #[test]
    fn decodes_systemd_cgroup_name() {
        let cg = "0::/machine.slice/machine-qemu\\x2d3\\x2dprod.scope";
        assert_eq!(parse_cgroup_domain_name(cg).as_deref(), Some("3-prod"));
    }

    #[test]
    fn parses_proc_starttime() {
        let stat = "123 (qemu system) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 9999 20";
        assert_eq!(parse_proc_stat_start_time(stat), Some(9999));
    }

    #[test]
    fn parses_mocked_libvirt_domain_xml() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/libvirt/win11.xml");
        let xml = std::fs::read_to_string(&path).unwrap();

        let domain = parse_libvirt_domain_xml(&path, &xml).unwrap();

        assert_eq!(domain.name, "win11");
        assert_eq!(domain.uuid, "11111111-2222-3333-4444-555555555555");
        assert_eq!(
            domain.qmp_socket.as_deref(),
            Some("/run/libvirt/qemu/win11.monitor")
        );
        assert_eq!(domain.qemu_pids, [4242]);
        assert_eq!(domain.qemu_tids, [4243, 4244]);
        assert_eq!(
            domain.qemu_task_identities,
            [
                HostTaskIdentity {
                    task_id: 4242,
                    start_time_ticks: None
                },
                HostTaskIdentity {
                    task_id: 4243,
                    start_time_ticks: None
                },
                HostTaskIdentity {
                    task_id: 4244,
                    start_time_ticks: None
                }
            ]
        );
        assert_eq!(
            domain.vcpu_threads,
            [
                QemuVcpuThread {
                    tid: 4243,
                    vcpu_id: 0
                },
                QemuVcpuThread {
                    tid: 4244,
                    vcpu_id: 1
                }
            ]
        );
    }

    #[test]
    fn libvirt_discovery_maps_qemu_tid_to_domain() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/libvirt");
        let discovery = LibvirtDomainDiscovery::from_dir(&dir).unwrap();

        let domain = discovery.lookup_task(4243).unwrap().unwrap();

        assert_eq!(domain.name, "win11");
        assert_eq!(domain.uuid, "11111111-2222-3333-4444-555555555555");
    }

    #[test]
    fn resolver_enriches_guest_vcpu_from_known_host_tid_mapping() {
        let host_pid = 2_000_000_042;
        let host_tid = 2_000_000_043;
        let mut domain = domain_with_task_start(host_tid, None);
        domain.name = "mapped-vcpu".to_string();
        domain.uuid = "11111111-2222-3333-4444-555555555555".to_string();
        domain.qemu_pids = vec![host_pid];
        domain.qemu_tids = vec![host_tid];
        domain.vcpu_threads = vec![QemuVcpuThread {
            tid: host_tid,
            vcpu_id: 0,
        }];

        let resolver = VmIdentityResolver::new(Identity::default()).unwrap();
        *resolver.libvirt.lock().unwrap() =
            Some(LibvirtDomainDiscovery::from_domains(vec![domain]));

        let mut ev = Event::base(
            Category::Exit,
            Severity::Info,
            "2026-01-01T00:00:00Z".to_string(),
            "qemu-system-x86".to_string(),
        );
        ev.host_pid = Some(host_pid);
        ev.host_tid = Some(host_tid);
        ev.host_cpu = Some(7);

        let enrichment = resolver.enrich_event(&mut ev);

        assert_eq!(
            ev.vm_id.as_deref(),
            Some("libvirt:11111111-2222-3333-4444-555555555555")
        );
        assert_eq!(enrichment.cache_result, Some(IdentityCacheResult::Miss));
        assert_eq!(enrichment.confidence, Some(IdentityConfidence::Medium));
        assert!(!enrichment.ambiguous);
        assert_eq!(enrichment.conflict_reason, None);
        assert_eq!(ev.vcpu_id, Some(0));
        assert_eq!(ev.vcpu, Some(0));
        assert_eq!(ev.host_cpu, Some(7));
        assert!(ev.tags.contains(&"identity:vcpu-map".to_string()));
    }

    #[test]
    fn resolver_does_not_overwrite_tracepoint_vcpu_id() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/libvirt");
        let cfg = Identity {
            libvirt_xml_dir: dir.display().to_string(),
            ..Identity::default()
        };
        let resolver = VmIdentityResolver::new(cfg).unwrap();
        let mut ev = Event::base(
            Category::Exit,
            Severity::Info,
            "2026-01-01T00:00:00Z".to_string(),
            "qemu-system-x86".to_string(),
        );
        ev.host_pid = Some(4242);
        ev.host_tid = Some(4243);
        ev.vcpu_id = Some(9);
        ev.vcpu = Some(9);

        resolver.enrich_event(&mut ev);

        assert_eq!(ev.vcpu_id, Some(9));
        assert_eq!(ev.vcpu, Some(9));
        assert!(!ev.tags.contains(&"identity:vcpu-map".to_string()));
    }

    #[test]
    fn resolver_leaves_vcpu_unknown_when_metadata_is_missing() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/libvirt");
        let cfg = Identity {
            libvirt_xml_dir: dir.display().to_string(),
            ..Identity::default()
        };
        let resolver = VmIdentityResolver::new(cfg).unwrap();
        let mut ev = Event::base(
            Category::Exit,
            Severity::Info,
            "2026-01-01T00:00:00Z".to_string(),
            "qemu-system-x86".to_string(),
        );
        ev.host_pid = Some(5252);
        ev.host_tid = Some(5253);

        resolver.enrich_event(&mut ev);

        assert_eq!(
            ev.vm_id.as_deref(),
            Some("libvirt:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
        );
        assert_eq!(ev.vcpu_id, None);
        assert_eq!(ev.vcpu, None);
        assert!(!ev.tags.contains(&"identity:vcpu-map".to_string()));
    }

    #[test]
    fn malformed_vcpu_thread_metadata_returns_typed_error() {
        let path = Path::new("inline-conflicting-vcpu.xml");
        let xml = r#"
<domain type="kvm">
  <name>bad-vcpu-map</name>
  <uuid>99999999-aaaa-bbbb-cccc-dddddddddddd</uuid>
  <metadata>
    <aegishv:qemu xmlns:aegishv="https://github.com/Nullbit1/AegisHV" pid="7000">
      <aegishv:thread tid="7001" vcpu_id="0"/>
      <aegishv:thread tid="7001" vcpu_id="1"/>
    </aegishv:qemu>
  </metadata>
</domain>
"#;

        let err = parse_libvirt_domain_xml(path, xml).unwrap_err();

        assert!(matches!(err, LibvirtDiscoveryError::MalformedDomain { .. }));
        assert!(err.to_string().contains("conflicting vCPU metadata"));
    }

    fn domain_with_task_start(task_id: i32, start_time_ticks: Option<u64>) -> LibvirtDomain {
        LibvirtDomain {
            name: "started-vm".to_string(),
            uuid: "77777777-8888-9999-aaaa-bbbbbbbbbbbb".to_string(),
            qmp_socket: Some("/run/libvirt/qemu/started-vm.monitor".to_string()),
            qemu_pids: vec![task_id],
            qemu_tids: Vec::new(),
            vcpu_threads: Vec::new(),
            qemu_task_identities: vec![HostTaskIdentity {
                task_id,
                start_time_ticks,
            }],
            source: LibvirtDomainSource::Xml,
        }
    }

    #[test]
    fn inventory_snapshot_reports_libvirt_domains_without_socket_paths() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/libvirt");
        let discovery = LibvirtDomainDiscovery::from_dir(&dir).unwrap();

        let snapshot = discovery.inventory_snapshot();
        let json = snapshot.to_json();
        let win11 = snapshot
            .vms
            .iter()
            .find(|vm| vm.vm_name == "win11")
            .expect("win11 inventory entry");

        assert_eq!(snapshot.status, "degraded");
        assert_eq!(snapshot.source, "libvirt_xml");
        assert_eq!(snapshot.freshness, "file_backed_snapshot");
        assert_eq!(snapshot.vm_count, 4);
        assert!(snapshot.degraded);
        assert_eq!(win11.vm_id, "libvirt:11111111-2222-3333-4444-555555555555");
        assert_eq!(win11.qmp.status, "configured");
        assert!(win11.qmp.present);
        assert_eq!(win11.vcpu_mappings.len(), 2);
        assert_eq!(win11.identity.confidence, IdentityConfidence::Medium);
        assert!(!win11.identity.start_time_verified);
        assert!(!json.contains("/run/"));
        assert!(!json.contains("<domain"));
        assert!(!json.contains("qemu-system"));
    }

    #[test]
    fn inventory_snapshot_marks_duplicate_host_task_domains_degraded() {
        let mut first = domain_with_task_start(4242, None);
        first.name = "first".to_string();
        first.uuid = "11111111-1111-1111-1111-111111111111".to_string();
        let mut second = domain_with_task_start(4242, None);
        second.name = "second".to_string();
        second.uuid = "22222222-2222-2222-2222-222222222222".to_string();
        let discovery = LibvirtDomainDiscovery::from_domains(vec![first, second]);

        let snapshot = discovery.inventory_snapshot();

        assert_eq!(snapshot.status, "degraded");
        assert!(snapshot.degraded);
        assert!(snapshot.vms.iter().all(|vm| vm.ambiguous));
        assert!(snapshot.vms.iter().all(|vm| {
            vm.conflict
                .as_ref()
                .map(|conflict| conflict.reason.as_str())
                == Some("multiple_domains")
        }));
    }

    #[test]
    fn disabled_identity_resolver_inventory_is_explicitly_disabled() {
        let resolver = VmIdentityResolver::new(Identity {
            enable: false,
            ..Identity::default()
        })
        .unwrap();

        let snapshot = resolver.inventory_snapshot();

        assert_eq!(snapshot.status, "disabled");
        assert_eq!(snapshot.source, "disabled");
        assert_eq!(snapshot.vm_count, 0);
        assert!(!snapshot.degraded);
    }

    #[test]
    fn libvirt_lookup_accepts_matching_process_start_time() {
        let discovery =
            LibvirtDomainDiscovery::from_domains(vec![domain_with_task_start(4242, Some(1111))]);

        let domain = discovery
            .lookup_task_with_start_time(4242, Some(1111))
            .unwrap()
            .unwrap();

        assert_eq!(domain.uuid, "77777777-8888-9999-aaaa-bbbbbbbbbbbb");
    }

    #[test]
    fn libvirt_lookup_rejects_pid_reuse_start_time_mismatch() {
        let discovery =
            LibvirtDomainDiscovery::from_domains(vec![domain_with_task_start(4242, Some(1111))]);

        let err = discovery
            .lookup_task_with_start_time(4242, Some(2222))
            .unwrap_err();

        assert!(matches!(
            err,
            LibvirtDiscoveryError::PidReuseDetected { .. }
        ));
        assert!(err.to_string().contains("rejected host task 4242"));
    }

    #[test]
    fn libvirt_lookup_rejects_start_aware_mapping_without_observed_start_time() {
        let discovery =
            LibvirtDomainDiscovery::from_domains(vec![domain_with_task_start(4242, Some(1111))]);

        let err = discovery
            .lookup_task_with_start_time(4242, None)
            .unwrap_err();

        assert!(matches!(
            err,
            LibvirtDiscoveryError::UnverifiedTaskIdentity { .. }
        ));
        assert!(err.to_string().contains("cannot verify host task 4242"));
    }

    #[test]
    fn resolver_marks_pid_reuse_detection_ambiguous_for_qmp_safety() {
        let discovery =
            LibvirtDomainDiscovery::from_domains(vec![domain_with_task_start(4242, Some(1111))]);
        let mut ident = VmIdentity {
            host_pid: 4242,
            host_start_time_ticks: Some(2222),
            vm_id: "host-pid:4242:start:2222".to_string(),
            ..VmIdentity::default()
        };

        apply_libvirt_discovery(4242, &mut ident, &discovery);

        assert!(ident.ambiguous);
        assert_eq!(ident.vm_id, "ambiguous:host-task:4242:pid-reuse");
        assert_eq!(ident.qmp_socket, None);
    }

    #[test]
    fn identity_conflict_event_is_bounded_and_cooldown_suppressed() {
        let resolver = VmIdentityResolver::new(Identity::default()).unwrap();
        let ident = VmIdentity {
            host_pid: 4242,
            host_start_time_ticks: Some(2222),
            vm_id: "ambiguous:host-task:4242:pid-reuse".to_string(),
            ambiguous: true,
            identity_sources: vec![IDENTITY_SOURCE_AMBIGUOUS.to_string()],
            identity_conflict: Some(IdentityConflict {
                task_id: 4242,
                reason: IdentityConflictReason::PidReuse,
                ambiguous: true,
            }),
            ..VmIdentity::default()
        };

        let first = resolver.identity_conflict_event(&ident).unwrap();
        let second = resolver.identity_conflict_event(&ident);

        assert_eq!(first.category, Category::Sensor);
        assert_eq!(first.reason.as_deref(), Some("identity_conflict"));
        assert_eq!(first.vm, "host");
        assert_eq!(
            first.tags,
            [
                "identity:conflict".to_string(),
                "identity_conflict:pid_reuse".to_string()
            ]
        );
        assert!(first
            .message
            .as_deref()
            .unwrap()
            .contains("reason=pid_reuse"));
        assert!(first
            .message
            .as_deref()
            .unwrap()
            .contains("outcome=identity_degraded"));
        assert!(!first.message.as_deref().unwrap().contains("/run/"));
        assert!(!first.message.as_deref().unwrap().contains("<domain"));
        assert!(!first.message.as_deref().unwrap().contains("qemu-system"));
        assert!(first.identity.as_ref().unwrap().ambiguous);
        assert!(second.is_none());
    }

    #[test]
    fn libvirt_qmp_socket_mismatch_marks_identity_ambiguous() {
        let discovery =
            LibvirtDomainDiscovery::from_domains(vec![domain_with_task_start(4242, Some(1111))]);
        let mut ident = VmIdentity {
            host_pid: 4242,
            host_start_time_ticks: Some(1111),
            vm_id: "host-pid:4242:start:1111".to_string(),
            qmp_socket: Some("/run/qemu/other.monitor".to_string()),
            ..VmIdentity::default()
        };

        apply_libvirt_discovery(4242, &mut ident, &discovery);

        assert!(ident.ambiguous);
        assert_eq!(ident.vm_id, "ambiguous:host-task:4242:qmp-socket-mismatch");
        assert_eq!(
            ident.identity_conflict.as_ref().unwrap().reason,
            IdentityConflictReason::QmpSocketMismatch
        );
        assert_eq!(ident.qmp_socket, None);
    }

    #[test]
    fn libvirt_uuid_mismatch_marks_identity_ambiguous() {
        let discovery =
            LibvirtDomainDiscovery::from_domains(vec![domain_with_task_start(4242, Some(1111))]);
        let mut ident = VmIdentity {
            host_pid: 4242,
            host_start_time_ticks: Some(1111),
            vm_id: "libvirt:00000000-1111-2222-3333-444444444444".to_string(),
            libvirt_uuid: Some("00000000-1111-2222-3333-444444444444".to_string()),
            ..VmIdentity::default()
        };

        apply_libvirt_discovery(4242, &mut ident, &discovery);

        assert!(ident.ambiguous);
        assert_eq!(
            ident.vm_id,
            "ambiguous:host-task:4242:libvirt-uuid-mismatch"
        );
        assert_eq!(
            ident.identity_conflict.as_ref().unwrap().reason,
            IdentityConflictReason::LibvirtUuidMismatch
        );
    }

    #[test]
    fn proc_cgroup_name_mismatch_without_uuid_marks_identity_ambiguous() {
        let mut ident = VmIdentity {
            host_pid: 4242,
            host_start_time_ticks: Some(1111),
            vm_id: "name:cmdline-vm".to_string(),
            vm_name: Some("cmdline-vm".to_string()),
            identity_sources: vec![IDENTITY_SOURCE_PROC_CMDLINE.to_string()],
            ..VmIdentity::default()
        };

        merge_cgroup_identity(
            &mut ident,
            4242,
            Some("cgroup-vm".to_string()),
            Some("machine-qemu-cgroup-vm.scope".to_string()),
        );

        assert!(ident.ambiguous);
        assert_eq!(ident.vm_id, "ambiguous:host-task:4242:proc-cgroup-mismatch");
        assert_eq!(
            ident.identity_conflict.as_ref().unwrap().reason,
            IdentityConflictReason::ProcCgroupMismatch
        );
    }

    #[test]
    fn libvirt_name_mismatch_emits_diagnostic_without_ambiguous_identity() {
        let discovery =
            LibvirtDomainDiscovery::from_domains(vec![domain_with_task_start(4242, Some(1111))]);
        let mut ident = VmIdentity {
            host_pid: 4242,
            host_start_time_ticks: Some(1111),
            vm_id: "name:old-name".to_string(),
            vm_name: Some("old-name".to_string()),
            identity_sources: vec![IDENTITY_SOURCE_PROC_CMDLINE.to_string()],
            ..VmIdentity::default()
        };

        apply_libvirt_discovery(4242, &mut ident, &discovery);

        assert!(!ident.ambiguous);
        assert_eq!(ident.vm_id, "libvirt:77777777-8888-9999-aaaa-bbbbbbbbbbbb");
        assert_eq!(
            ident.identity_conflict.as_ref().unwrap().reason,
            IdentityConflictReason::LibvirtNameMismatch
        );
    }

    #[test]
    fn start_time_verified_libvirt_identity_reports_high_confidence() {
        let discovery =
            LibvirtDomainDiscovery::from_domains(vec![domain_with_task_start(4242, Some(1111))]);
        let mut ident = VmIdentity {
            host_pid: 4242,
            host_start_time_ticks: Some(1111),
            vm_id: "host-pid:4242:start:1111".to_string(),
            identity_sources: vec![IDENTITY_SOURCE_FALLBACK_PID.to_string()],
            ..VmIdentity::default()
        };

        apply_libvirt_discovery(4242, &mut ident, &discovery);

        assert_eq!(ident.identity_confidence, IdentityConfidence::High);
        assert!(ident.start_time_verified);
        assert!(ident
            .identity_sources
            .contains(&IDENTITY_SOURCE_LIBVIRT_XML.to_string()));
        assert!(ident
            .identity_sources
            .contains(&IDENTITY_SOURCE_START_TIME_VERIFIED.to_string()));
    }

    #[test]
    fn ambiguous_identity_reports_low_confidence_source() {
        let discovery =
            LibvirtDomainDiscovery::from_domains(vec![domain_with_task_start(4242, Some(1111))]);
        let mut ident = VmIdentity {
            host_pid: 4242,
            host_start_time_ticks: Some(2222),
            vm_id: "host-pid:4242:start:2222".to_string(),
            ..VmIdentity::default()
        };

        apply_libvirt_discovery(4242, &mut ident, &discovery);

        assert_eq!(ident.identity_confidence, IdentityConfidence::Low);
        assert_eq!(
            ident.identity_sources,
            vec![IDENTITY_SOURCE_AMBIGUOUS.to_string()]
        );
        assert!(!ident.start_time_verified);
    }

    #[test]
    fn proc_cmdline_uuid_identity_is_medium_confidence_not_high() {
        let mut ident = VmIdentity {
            host_pid: 4242,
            host_start_time_ticks: Some(1111),
            vm_id: "libvirt:11111111-2222-3333-4444-555555555555".to_string(),
            libvirt_uuid: Some("11111111-2222-3333-4444-555555555555".to_string()),
            identity_sources: vec![IDENTITY_SOURCE_PROC_CMDLINE.to_string()],
            ..VmIdentity::default()
        };

        recompute_identity_confidence(&mut ident);

        assert_eq!(ident.identity_confidence, IdentityConfidence::Medium);
        assert!(!ident.start_time_verified);
    }

    #[test]
    fn identity_cache_requires_matching_observed_start_time() {
        let now = Instant::now();
        let cached = CachedIdentity {
            seen_at: now,
            start_time_ticks: Some(100),
            value: VmIdentity {
                host_pid: 4242,
                host_start_time_ticks: Some(100),
                vm_id: "host-pid:4242:start:100".to_string(),
                ..VmIdentity::default()
            },
        };

        assert!(cache_entry_matches(
            &cached,
            Some(100),
            now + Duration::from_millis(10),
            Duration::from_millis(100)
        ));
        assert!(!cache_entry_matches(
            &cached,
            Some(101),
            now + Duration::from_millis(10),
            Duration::from_millis(100)
        ));
        assert!(!cache_entry_matches(
            &cached,
            None,
            now + Duration::from_millis(10),
            Duration::from_millis(100)
        ));
        assert!(!cache_entry_matches(
            &cached,
            Some(100),
            now + Duration::from_millis(101),
            Duration::from_millis(100)
        ));
    }

    #[test]
    fn resolver_does_not_cache_unversioned_pid_only_identity() {
        let resolver = VmIdentityResolver::new(Identity::default()).unwrap();

        let _ = resolver.resolve_pid(2_147_000_001);

        assert_eq!(resolver.cache_len(), 0);
    }

    #[test]
    fn deterministic_replay_resolver_does_not_read_live_host_identity() {
        let resolver = VmIdentityResolver::deterministic_replay(Identity::default()).unwrap();

        let ident = resolver.resolve_pid(4242);

        assert_eq!(ident.vm_id, "host-pid:4242");
        assert_eq!(ident.host_start_time_ticks, None);
        assert_eq!(
            ident.identity_sources,
            vec![IDENTITY_SOURCE_FALLBACK_PID.to_string()]
        );
    }

    #[test]
    fn resolver_enriches_from_optional_libvirt_xml_discovery() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/libvirt");
        let cfg = Identity {
            libvirt_xml_dir: dir.display().to_string(),
            ..Identity::default()
        };
        let resolver = VmIdentityResolver::new(cfg).unwrap();

        let ident = resolver.resolve_pid(5253);

        assert_eq!(ident.vm_id, "libvirt:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        assert_eq!(ident.vm_name.as_deref(), Some("prod-linux"));
        assert_eq!(
            ident.qmp_socket.as_deref(),
            Some("/run/libvirt/qemu/prod-linux.monitor")
        );
        assert!(!ident.ambiguous);
    }

    #[test]
    fn ambiguous_libvirt_task_marks_identity_ambiguous() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/libvirt");
        let cfg = Identity {
            libvirt_xml_dir: dir.display().to_string(),
            ..Identity::default()
        };
        let resolver = VmIdentityResolver::new(cfg).unwrap();

        let ident = resolver.resolve_pid(6262);

        assert_eq!(ident.vm_id, "ambiguous:host-task:6262");
        assert!(ident.ambiguous);
        assert_eq!(ident.vm_name, None);
        assert_eq!(ident.qmp_socket, None);
    }

    #[test]
    fn malformed_libvirt_fixture_returns_typed_error() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/libvirt-bad/malformed-missing-uuid.xml");
        let xml = std::fs::read_to_string(&path).unwrap();

        let err = parse_libvirt_domain_xml(&path, &xml).unwrap_err();

        assert!(matches!(err, LibvirtDiscoveryError::MalformedDomain { .. }));
        assert!(err.to_string().contains("missing required <uuid>"));
    }

    #[test]
    fn lifecycle_source_start_event_refreshes_discovery_and_emits_sensor_event() {
        let resolver = VmIdentityResolver::new(Identity::default()).unwrap();
        let mut source = QueuedLibvirtLifecycleSource::new(vec![Ok(LibvirtLifecycleEvent {
            kind: LibvirtLifecycleKind::Started,
            uuid: "77777777-8888-9999-aaaa-bbbbbbbbbbbb".to_string(),
            name: Some("started-vm".to_string()),
            qemu_pid: Some(9001),
            qemu_tids: vec![9002],
            vcpu_threads: vec![QemuVcpuThread {
                tid: 9002,
                vcpu_id: 3,
            }],
            qmp_socket: Some("/run/libvirt/qemu/started-vm.monitor".to_string()),
        })]);

        let events = resolver
            .drain_libvirt_lifecycle(&mut source)
            .expect("queued lifecycle source must drain");
        let ident = resolver.resolve_pid(9002);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].category, Category::Sensor);
        assert_eq!(events[0].reason.as_deref(), Some("libvirt_lifecycle"));
        assert!(events[0].tags.contains(&"identity:libvirt".to_string()));
        let identity = events[0].identity.as_ref().unwrap();
        assert_eq!(identity.confidence, IdentityConfidence::Medium);
        assert_eq!(
            identity.sources,
            vec![IDENTITY_SOURCE_LIBVIRT_LIFECYCLE.to_string()]
        );
        assert!(events[0]
            .message
            .as_deref()
            .unwrap()
            .contains("kind=started"));
        assert_eq!(ident.vm_id, "libvirt:77777777-8888-9999-aaaa-bbbbbbbbbbbb");
        assert_eq!(ident.vm_name.as_deref(), Some("started-vm"));
        assert_eq!(
            ident.qmp_socket.as_deref(),
            Some("/run/libvirt/qemu/started-vm.monitor")
        );
        assert_eq!(ident.vcpu_id, Some(3));
    }

    #[test]
    fn lifecycle_stop_removes_discovery_and_invalidates_cached_identity() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/libvirt");
        let resolver = VmIdentityResolver::new(Identity {
            libvirt_xml_dir: dir.display().to_string(),
            ..Identity::default()
        })
        .unwrap();
        let before = resolver.resolve_pid(5253);

        let event = resolver
            .apply_libvirt_lifecycle_event(LibvirtLifecycleEvent {
                kind: LibvirtLifecycleKind::Stopped,
                uuid: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string(),
                name: Some("prod-linux".to_string()),
                qemu_pid: Some(5252),
                qemu_tids: vec![5253],
                vcpu_threads: Vec::new(),
                qmp_socket: None,
            })
            .expect("stop lifecycle event must apply");
        let after = resolver.resolve_pid(5253);

        assert_eq!(before.vm_id, "libvirt:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        assert!(!after.vm_id.starts_with("libvirt:"));
        let message = event.message.as_deref().unwrap();
        assert!(message.contains("kind=stopped"));
        assert!(message.contains("discovery=removed"));
        assert!(message.contains("cache_invalidated=0"));
    }

    #[test]
    fn lifecycle_update_clears_stale_task_mapping_before_resolution() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/libvirt");
        let resolver = VmIdentityResolver::new(Identity {
            libvirt_xml_dir: dir.display().to_string(),
            ..Identity::default()
        })
        .unwrap();
        let before = resolver.resolve_pid(6262);

        resolver
            .apply_libvirt_lifecycle_event(LibvirtLifecycleEvent {
                kind: LibvirtLifecycleKind::Started,
                uuid: "bbbbbbbb-cccc-dddd-eeee-ffffffffffff".to_string(),
                name: Some("ambiguous-b".to_string()),
                qemu_pid: Some(6262),
                qemu_tids: Vec::new(),
                vcpu_threads: Vec::new(),
                qmp_socket: Some("/run/libvirt/qemu/ambiguous-b.monitor".to_string()),
            })
            .expect("lifecycle update must replace stale task mapping");
        let after = resolver.resolve_pid(6262);

        assert!(before.ambiguous);
        assert!(!after.ambiguous);
        assert_eq!(after.vm_id, "libvirt:bbbbbbbb-cccc-dddd-eeee-ffffffffffff");
        assert_eq!(after.vm_name.as_deref(), Some("ambiguous-b"));
    }

    #[test]
    fn lifecycle_pause_invalidates_cache_without_removing_discovery() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/libvirt");
        let resolver = VmIdentityResolver::new(Identity {
            libvirt_xml_dir: dir.display().to_string(),
            ..Identity::default()
        })
        .unwrap();
        let before = resolver.resolve_pid(5253);

        let event = resolver
            .apply_libvirt_lifecycle_event(LibvirtLifecycleEvent {
                kind: LibvirtLifecycleKind::Paused,
                uuid: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string(),
                name: Some("prod-linux".to_string()),
                qemu_pid: None,
                qemu_tids: Vec::new(),
                vcpu_threads: Vec::new(),
                qmp_socket: None,
            })
            .expect("pause lifecycle event must apply");
        let after = resolver.resolve_pid(5253);

        assert_eq!(before.vm_id, "libvirt:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        assert_eq!(after.vm_id, "libvirt:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        let message = event.message.as_deref().unwrap();
        assert!(message.contains("kind=paused"));
        assert!(message.contains("discovery=unchanged"));
        assert!(message.contains("cache_invalidated=0"));
    }

    #[test]
    fn lifecycle_update_without_task_mapping_does_not_invent_identity() {
        let resolver = VmIdentityResolver::new(Identity::default()).unwrap();

        let event = resolver
            .apply_libvirt_lifecycle_event(LibvirtLifecycleEvent {
                kind: LibvirtLifecycleKind::Migrated,
                uuid: "cccccccc-dddd-eeee-ffff-000000000000".to_string(),
                name: Some("migrated-vm".to_string()),
                qemu_pid: None,
                qemu_tids: Vec::new(),
                vcpu_threads: Vec::new(),
                qmp_socket: None,
            })
            .expect("mapping-free lifecycle event must be explicit but non-fatal");
        let ident = resolver.resolve_pid(9100);

        assert!(!ident.vm_id.starts_with("libvirt:"));
        let message = event.message.as_deref().unwrap();
        assert!(message.contains("kind=migrated"));
        assert!(message.contains("discovery=unchanged"));
        assert!(message.contains("task_count=0"));
    }

    #[test]
    fn lifecycle_event_requires_domain_uuid() {
        let resolver = VmIdentityResolver::new(Identity::default()).unwrap();

        let err = resolver
            .apply_libvirt_lifecycle_event(LibvirtLifecycleEvent {
                kind: LibvirtLifecycleKind::Started,
                uuid: " ".to_string(),
                name: Some("missing-uuid".to_string()),
                qemu_pid: Some(1),
                qemu_tids: Vec::new(),
                vcpu_threads: Vec::new(),
                qmp_socket: None,
            })
            .expect_err("missing UUID must be rejected");

        assert!(matches!(err, LibvirtLifecycleError::MalformedEvent(_)));
        assert!(err.to_string().contains("missing domain uuid"));
    }

    #[test]
    fn lifecycle_source_error_is_typed_and_does_not_drain_as_success() {
        let resolver = VmIdentityResolver::new(Identity::default()).unwrap();
        let mut source = QueuedLibvirtLifecycleSource::new(vec![Err(
            LibvirtLifecycleError::Source("mock source closed".to_string()),
        )]);

        let err = resolver
            .drain_libvirt_lifecycle(&mut source)
            .expect_err("source error must stop lifecycle drain");

        assert!(matches!(err, LibvirtLifecycleError::Source(_)));
        assert!(err.to_string().contains("mock source closed"));
    }

    #[test]
    fn unsupported_lifecycle_kind_is_typed() {
        let err = LibvirtLifecycleKind::try_from("crashed")
            .expect_err("unsupported lifecycle kind must be explicit");

        assert!(matches!(err, LibvirtLifecycleError::UnsupportedKind(_)));
        assert!(err.to_string().contains("crashed"));
    }
}
