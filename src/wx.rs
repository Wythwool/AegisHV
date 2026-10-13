use crate::config::Config;
use crate::event::{AddrInfo, Category, Event, Severity, TrapInfo, ViolationBits, WxInfo};
use crate::identity::IdentityConflictReason;
use crate::pattern::Pattern;
use crate::util::{format_hex, now_rfc3339, page_align, parse_hex_u64};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
struct PageState {
    last_write_at: Option<Instant>,
    last_write_rip: Option<String>,
    last_write_identity_conflict_tags: Vec<String>,
    last_exec_at: Option<Instant>,
    last_exec_rip: Option<String>,
    last_alert_by_reason: HashMap<String, Instant>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct PageKey {
    vm_scope: String,
    address_space: String,
    gpa_page: u64,
}

#[derive(Debug, Clone)]
struct AllowEntry {
    vm: Option<Pattern>,
    gpa_prefix: String,
}

pub struct WxEngine {
    pages: Mutex<HashMap<PageKey, PageState>>,
    window: Duration,
    cooldown: Duration,
    max_pages: usize,
    page_size: u64,
    allow: Vec<AllowEntry>,
    trap_mode: WxTrapMode,
    pruned_delta: std::sync::atomic::AtomicU64,
    cooldown_suppressed_delta: std::sync::atomic::AtomicU64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WxTrapMode {
    CorrelationOnly,
    TrapEnforced {
        backend: String,
        invalidation_status: String,
    },
}

impl WxEngine {
    pub fn new(cfg: &Config) -> Self {
        Self::new_with_trap_mode(cfg, WxTrapMode::CorrelationOnly)
    }

    pub fn new_with_trap_mode(cfg: &Config, trap_mode: WxTrapMode) -> Self {
        let allow = cfg
            .wx_allow
            .entries
            .iter()
            .map(|entry| AllowEntry {
                vm: if entry.vm.trim().is_empty() {
                    None
                } else {
                    Pattern::compile(&entry.vm).ok()
                },
                gpa_prefix: entry.gpa_prefix.clone(),
            })
            .collect();
        Self {
            pages: Mutex::new(HashMap::new()),
            window: Duration::from_millis(cfg.general.wx_window_ms),
            cooldown: Duration::from_millis(cfg.general.wx_cooldown_ms),
            max_pages: cfg.general.wx_max_pages,
            page_size: cfg.general.page_size,
            allow,
            trap_mode,
            pruned_delta: std::sync::atomic::AtomicU64::new(0),
            cooldown_suppressed_delta: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub fn pages_tracked(&self) -> usize {
        self.pages.lock().map(|p| p.len()).unwrap_or(0)
    }

    pub fn take_pruned_delta(&self) -> u64 {
        self.pruned_delta
            .swap(0, std::sync::atomic::Ordering::Relaxed)
    }

    pub fn take_cooldown_suppressed_delta(&self) -> u64 {
        self.cooldown_suppressed_delta
            .swap(0, std::sync::atomic::Ordering::Relaxed)
    }

    pub fn on_exit_event(&self, ev: &Event) -> Option<Event> {
        if ev.category != Category::Exit {
            return None;
        }
        let bits = ev.violation?;
        if !bits.write && !bits.exec {
            return None;
        }
        let addr = ev.addr.as_ref()?;
        let gpa_raw = addr.gpa.as_ref()?;
        let gpa = parse_hex_u64(gpa_raw)?;
        let gpa_page = page_align(gpa, self.page_size);
        let page_hex = format_hex(gpa_page);
        if self.is_allowed(ev, &page_hex) {
            return None;
        }
        let vm_scope = ev.vm_id.clone().unwrap_or_else(|| ev.vm.clone());
        let address_space = ev
            .cr3
            .clone()
            .or_else(|| ev.asid.clone())
            .or_else(|| ev.vmid.clone())
            .or_else(|| ev.vpid.clone())
            .unwrap_or_else(|| "global".to_string());
        let key = PageKey {
            vm_scope: vm_scope.clone(),
            address_space: address_space.clone(),
            gpa_page,
        };
        let now = Instant::now();
        let mut pages = self.pages.lock().ok()?;
        let st = pages.entry(key).or_default();
        if bits.write {
            st.last_write_at = Some(now);
            st.last_write_rip = addr.rip.clone();
            st.last_write_identity_conflict_tags = safe_identity_conflict_tags(&ev.tags);
        }
        if bits.exec {
            st.last_exec_at = Some(now);
            st.last_exec_rip = addr.rip.clone();
            if let Some(w_at) = st.last_write_at {
                let delta = now.saturating_duration_since(w_at);
                if delta <= self.window {
                    let cooldown_reason = ev.reason.as_deref().unwrap_or("W^X");
                    if self.cooldown_active(st, cooldown_reason, now) {
                        self.maybe_prune(&mut pages);
                        return None;
                    }
                    let confidence = (1.0
                        - (delta.as_secs_f64() / self.window.as_secs_f64()) * 0.5)
                        .clamp(0.1, 1.0);
                    let mut wx_ev = Event::base(
                        Category::Wx,
                        Severity::Critical,
                        now_rfc3339(),
                        ev.vm.clone(),
                    );
                    wx_ev.vm_id = ev.vm_id.clone();
                    wx_ev.vm_name = ev.vm_name.clone();
                    wx_ev.raw_comm = ev.raw_comm.clone();
                    wx_ev.host_pid = ev.host_pid;
                    wx_ev.host_tid = ev.host_tid;
                    wx_ev.host_start_time_ticks = ev.host_start_time_ticks;
                    wx_ev.identity = ev.identity.clone();
                    wx_ev.host_cpu = ev.host_cpu;
                    wx_ev.vcpu_id = ev.vcpu_id;
                    wx_ev.vcpu = ev.vcpu;
                    wx_ev.cr3 = ev.cr3.clone();
                    wx_ev.asid = ev.asid.clone();
                    wx_ev.vmid = ev.vmid.clone();
                    wx_ev.vpid = ev.vpid.clone();
                    wx_ev.arch = ev.arch.clone();
                    wx_ev.guest_os = ev.guest_os.clone();
                    wx_ev.guest_process = ev.guest_process.clone();
                    wx_ev.guest_thread = ev.guest_thread.clone();
                    wx_ev.guest_module = ev.guest_module.clone();
                    wx_ev.guest_symbol = ev.guest_symbol.clone();
                    wx_ev.reason = Some("W^X".to_string());
                    wx_ev.trap_type = ev.trap_type.clone();
                    wx_ev.message = Some(
                        match self.trap_mode {
                            WxTrapMode::CorrelationOnly => {
                                "write then exec on same guest page within correlation window"
                            }
                            WxTrapMode::TrapEnforced { .. } => {
                                "write then exec on same guest page observed in trap-engine mode"
                            }
                        }
                        .to_string(),
                    );
                    wx_ev.tags = wx_tags_from_exit_event(
                        &self.trap_mode,
                        &st.last_write_identity_conflict_tags,
                        &ev.tags,
                    );
                    wx_ev.correlation_id =
                        Some(format!("wx:{}:{}:{:#x}", vm_scope, address_space, gpa_page));
                    wx_ev.addr = Some(AddrInfo {
                        rip: addr.rip.clone(),
                        gva: addr.gva.clone(),
                        gpa: Some(page_hex.clone()),
                        qual: addr.qual.clone(),
                    });
                    wx_ev.wx = Some(WxInfo {
                        writer_rip: st.last_write_rip.clone(),
                        executor_rip: st.last_exec_rip.clone(),
                        delta_ms: delta.as_millis() as u64,
                        page_size: Some(self.page_size),
                        confidence,
                    });
                    wx_ev.trap =
                        trap_info_from_mode(&self.trap_mode, &wx_ev.correlation_id, &page_hex);
                    self.maybe_prune(&mut pages);
                    return Some(wx_ev);
                }
            }
        }
        self.maybe_prune(&mut pages);
        None
    }

    fn is_allowed(&self, ev: &Event, gpa_page: &str) -> bool {
        self.allow.iter().any(|a| {
            let vm_ok =
                a.vm.as_ref()
                    .map(|p| {
                        p.is_match(&ev.vm)
                            || ev
                                .vm_id
                                .as_deref()
                                .map(|id| p.is_match(id))
                                .unwrap_or(false)
                    })
                    .unwrap_or(true);
            let gpa_ok = a.gpa_prefix.is_empty() || gpa_page.starts_with(&a.gpa_prefix);
            vm_ok && gpa_ok
        })
    }

    fn cooldown_active(&self, st: &mut PageState, reason: &str, now: Instant) -> bool {
        if self.cooldown.is_zero() {
            return false;
        }
        if let Some(prev) = st.last_alert_by_reason.get(reason) {
            if now.saturating_duration_since(*prev) < self.cooldown {
                self.cooldown_suppressed_delta
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return true;
            }
        }
        st.last_alert_by_reason.insert(reason.to_string(), now);
        false
    }

    fn maybe_prune(&self, pages: &mut HashMap<PageKey, PageState>) {
        if pages.len() <= self.max_pages {
            return;
        }
        let before = pages.len();
        let now = Instant::now();
        let window = self.window;
        pages.retain(|_, st| {
            let last = st.last_exec_at.or(st.last_write_at);
            last.map(|t| now.saturating_duration_since(t) <= window * 2)
                .unwrap_or(false)
        });
        if pages.len() > self.max_pages {
            let mut keys = pages
                .keys()
                .take(pages.len() - self.max_pages)
                .cloned()
                .collect::<Vec<_>>();
            for k in keys.drain(..) {
                pages.remove(&k);
            }
        }
        let pruned = before.saturating_sub(pages.len()) as u64;
        self.pruned_delta
            .fetch_add(pruned, std::sync::atomic::Ordering::Relaxed);
    }
}

fn wx_tags_from_exit_event(
    mode: &WxTrapMode,
    write_tags: &[String],
    source_tags: &[String],
) -> Vec<String> {
    let mut tags = vec!["wx".to_string(), "correlated".to_string()];
    match mode {
        WxTrapMode::CorrelationOnly => tags.push("not-enforcement".to_string()),
        WxTrapMode::TrapEnforced { .. } => tags.push("trap-enforcement".to_string()),
    }
    append_safe_identity_conflict_tags(&mut tags, write_tags);
    append_safe_identity_conflict_tags(&mut tags, source_tags);
    tags
}

fn trap_info_from_mode(
    mode: &WxTrapMode,
    correlation_id: &Option<String>,
    page_hex: &str,
) -> Option<TrapInfo> {
    match mode {
        WxTrapMode::CorrelationOnly => None,
        WxTrapMode::TrapEnforced {
            backend,
            invalidation_status,
        } => Some(TrapInfo {
            trap_id: correlation_id
                .clone()
                .unwrap_or_else(|| format!("trap:wx:{page_hex}")),
            trap_kind: "wx_correlation".to_string(),
            backend: backend.clone(),
            page: page_hex.to_string(),
            permissions_before: Some(ViolationBits {
                read: true,
                write: false,
                exec: false,
            }),
            permissions_after: Some(ViolationBits {
                read: true,
                write: false,
                exec: true,
            }),
            decision: "allow_step".to_string(),
            invalidation_status: invalidation_status.clone(),
        }),
    }
}

fn safe_identity_conflict_tags(source_tags: &[String]) -> Vec<String> {
    let mut tags = Vec::new();
    append_safe_identity_conflict_tags(&mut tags, source_tags);
    tags
}

fn append_safe_identity_conflict_tags(tags: &mut Vec<String>, source_tags: &[String]) {
    for tag in source_tags {
        if is_safe_identity_conflict_tag(tag) && tags.iter().all(|existing| existing != tag) {
            tags.push(tag.clone());
        }
    }
}

fn is_safe_identity_conflict_tag(tag: &str) -> bool {
    if tag == "identity:conflict" {
        return true;
    }
    let Some(reason) = tag.strip_prefix("identity_conflict:") else {
        return false;
    };
    IdentityConflictReason::ALL
        .iter()
        .any(|candidate| candidate.as_str() == reason)
}
