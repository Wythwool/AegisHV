use aegishv::config::{Action, Config, Match, Policy, QmpMapping, Rule};
use aegishv::event::{
    AddrInfo, Category, Event, IdentityConfidence, IdentityInfo, Severity, ViolationBits,
};
use aegishv::metrics::Metrics;
use aegishv::policy::PolicyEngine;
use aegishv::wx::{WxEngine, WxTrapMode};

fn base_exit(
    vm: &str,
    vm_id: Option<&str>,
    cr3: Option<&str>,
    gpa: &str,
    write: bool,
    exec: bool,
) -> Event {
    base_exit_with_reason(vm, vm_id, cr3, gpa, write, exec, None)
}

fn base_exit_with_reason(
    vm: &str,
    vm_id: Option<&str>,
    cr3: Option<&str>,
    gpa: &str,
    write: bool,
    exec: bool,
    reason: Option<&str>,
) -> Event {
    let mut ev = Event::base(
        Category::Exit,
        Severity::Info,
        "2026-01-01T00:00:00Z".to_string(),
        vm.to_string(),
    );
    ev.vm_id = vm_id.map(|s| s.to_string());
    ev.cr3 = cr3.map(|s| s.to_string());
    ev.addr = Some(AddrInfo {
        rip: Some("0xdeadbeef".to_string()),
        gva: None,
        gpa: Some(gpa.to_string()),
        qual: Some("0x0".to_string()),
    });
    ev.violation = Some(ViolationBits {
        read: false,
        write,
        exec,
    });
    ev.reason = reason.map(|value| value.to_string());
    ev
}

fn trusted_identity() -> IdentityInfo {
    IdentityInfo {
        sources: vec!["libvirt_xml".to_string(), "start_time_verified".to_string()],
        confidence: IdentityConfidence::High,
        start_time_verified: true,
        ambiguous: false,
    }
}

fn enforce_pause_on_wx_config() -> Config {
    let mut cfg = Config::default();
    cfg.actions.retries = 0;
    cfg.actions.qmp = vec![QmpMapping {
        vm_id: "libvirt:vm-a".to_string(),
        vm: String::new(),
        socket: "/run/libvirt/qemu/aegishv-test.monitor".to_string(),
    }];
    cfg.policy = Policy {
        rules: vec![Rule {
            name: "pause-on-wx".to_string(),
            id: "pause-on-wx".to_string(),
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
            cooldown_ms: 0,
            priority: 100,
            mode: "enforce".to_string(),
            enabled: true,
        }],
    };
    cfg
}

#[test]
fn correlates_same_vm_same_page() {
    let cfg = Config::default();
    let wx = WxEngine::new(&cfg);

    assert!(wx
        .on_exit_event(&base_exit(
            "vm-a",
            Some("vm-a"),
            Some("0xabc"),
            "0x1001",
            true,
            false
        ))
        .is_none());
    let alert = wx.on_exit_event(&base_exit(
        "vm-a",
        Some("vm-a"),
        Some("0xabc"),
        "0x1abc",
        false,
        true,
    ));
    assert!(alert.is_some());
    assert_eq!(
        alert.unwrap().addr.as_ref().unwrap().gpa.as_deref(),
        Some("0x1000")
    );
}

#[test]
fn wx_alert_preserves_identity_metadata_from_exit_event() {
    let cfg = Config::default();
    let wx = WxEngine::new(&cfg);
    let mut write = base_exit(
        "vm-a",
        Some("libvirt:vm-a"),
        Some("0xabc"),
        "0x1001",
        true,
        false,
    );
    write.identity = Some(IdentityInfo {
        sources: vec!["libvirt_xml".to_string(), "start_time_verified".to_string()],
        confidence: IdentityConfidence::High,
        start_time_verified: true,
        ambiguous: false,
    });
    assert!(wx.on_exit_event(&write).is_none());
    let mut exec = base_exit(
        "vm-a",
        Some("libvirt:vm-a"),
        Some("0xabc"),
        "0x1abc",
        false,
        true,
    );
    exec.identity = write.identity.clone();

    let alert = wx.on_exit_event(&exec).unwrap();

    assert_eq!(alert.identity, exec.identity);
}

#[test]
fn wx_alert_preserves_bounded_identity_conflict_tags() {
    let cfg = Config::default();
    let wx = WxEngine::new(&cfg);
    let mut write = base_exit(
        "vm-a",
        Some("libvirt:vm-a"),
        Some("0xabc"),
        "0x1001",
        true,
        false,
    );
    write.identity = Some(trusted_identity());
    write.tags = vec![
        "identity:conflict".to_string(),
        "identity_conflict:stale_cache".to_string(),
        "identity_conflict:/run/libvirt/qemu/aegishv-test.monitor".to_string(),
        "identity:qmp-hint".to_string(),
        "/run/libvirt/qemu/aegishv-test.monitor".to_string(),
        "qemu-system-x86".to_string(),
    ];
    assert!(wx.on_exit_event(&write).is_none());
    let mut exec = base_exit(
        "vm-a",
        Some("libvirt:vm-a"),
        Some("0xabc"),
        "0x1abc",
        false,
        true,
    );
    exec.identity = write.identity.clone();

    let alert = wx.on_exit_event(&exec).unwrap();

    assert!(alert.tags.contains(&"wx".to_string()));
    assert!(alert.tags.contains(&"correlated".to_string()));
    assert!(alert.tags.contains(&"not-enforcement".to_string()));
    assert!(alert.tags.contains(&"identity:conflict".to_string()));
    assert!(alert
        .tags
        .contains(&"identity_conflict:stale_cache".to_string()));
    assert!(!alert
        .tags
        .contains(&"identity_conflict:/run/libvirt/qemu/aegishv-test.monitor".to_string()));
    assert!(!alert.tags.contains(&"identity:qmp-hint".to_string()));
    assert!(!alert
        .tags
        .contains(&"/run/libvirt/qemu/aegishv-test.monitor".to_string()));
    assert!(!alert.tags.contains(&"qemu-system-x86".to_string()));
}

#[test]
fn trap_mode_marks_wx_event_as_enforcement_observed() {
    let cfg = Config::default();
    let wx = WxEngine::new_with_trap_mode(
        &cfg,
        WxTrapMode::TrapEnforced {
            backend: "synthetic".to_string(),
            invalidation_status: "recorded".to_string(),
        },
    );
    assert!(wx
        .on_exit_event(&base_exit(
            "vm-a",
            Some("libvirt:vm-a"),
            Some("0xabc"),
            "0x1001",
            true,
            false,
        ))
        .is_none());

    let alert = wx
        .on_exit_event(&base_exit(
            "vm-a",
            Some("libvirt:vm-a"),
            Some("0xabc"),
            "0x1abc",
            false,
            true,
        ))
        .unwrap();

    assert!(alert.tags.contains(&"trap-enforcement".to_string()));
    assert!(!alert.tags.contains(&"not-enforcement".to_string()));
    let trap = alert.trap.as_ref().unwrap();
    assert_eq!(trap.backend, "synthetic");
    assert_eq!(trap.trap_kind, "wx_correlation");
    assert_eq!(trap.page, "0x1000");
    assert_eq!(trap.invalidation_status, "recorded");
}

#[test]
fn wx_policy_action_refuses_stale_identity_conflict_before_qmp() {
    let cfg = enforce_pause_on_wx_config();
    let wx = WxEngine::new(&cfg);
    let mut write = base_exit(
        "vm-a",
        Some("libvirt:vm-a"),
        Some("0xabc"),
        "0x1001",
        true,
        false,
    );
    write.identity = Some(trusted_identity());
    write.tags = vec![
        "identity:conflict".to_string(),
        "identity_conflict:stale_cache".to_string(),
    ];
    assert!(wx.on_exit_event(&write).is_none());
    let mut exec = base_exit(
        "vm-a",
        Some("libvirt:vm-a"),
        Some("0xabc"),
        "0x1abc",
        false,
        true,
    );
    exec.identity = write.identity.clone();
    let alert = wx.on_exit_event(&exec).unwrap();
    let engine = PolicyEngine::new(&cfg).unwrap();
    let metrics = Metrics::new().unwrap();

    let out = engine.apply(&metrics, &alert);

    assert_eq!(out.len(), 1);
    let action = out[0].action.as_ref().unwrap();
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
        .contains("reason=stale_identity"));
    assert!(metrics
        .encode()
        .contains("aegishv_identity_qmp_safety_refusals_total{reason=\"stale_identity\"} 1"));
}

#[test]
fn does_not_cross_correlate_between_vms() {
    let cfg = Config::default();
    let wx = WxEngine::new(&cfg);

    assert!(wx
        .on_exit_event(&base_exit(
            "vm-a",
            Some("vm-a"),
            Some("0xabc"),
            "0x1000",
            true,
            false
        ))
        .is_none());
    let alert = wx.on_exit_event(&base_exit(
        "vm-b",
        Some("vm-b"),
        Some("0xabc"),
        "0x1000",
        false,
        true,
    ));
    assert!(alert.is_none());
}

#[test]
fn does_not_cross_correlate_between_address_spaces() {
    let cfg = Config::default();
    let wx = WxEngine::new(&cfg);

    assert!(wx
        .on_exit_event(&base_exit(
            "vm-a",
            Some("vm-a"),
            Some("0x111"),
            "0x2000",
            true,
            false
        ))
        .is_none());
    let alert = wx.on_exit_event(&base_exit(
        "vm-a",
        Some("vm-a"),
        Some("0x222"),
        "0x2000",
        false,
        true,
    ));
    assert!(alert.is_none());
}

#[test]
fn wx_alert_preserves_guest_attribution_from_exit_event() {
    let cfg = Config::default();
    let wx = WxEngine::new(&cfg);
    let mut write = base_exit("vm-a", Some("vm-a"), Some("0xabc"), "0x2400", true, false);
    write.guest_os = Some("linux".to_string());
    write.guest_process = Some("sshd".to_string());
    write.guest_thread = Some("sshd-worker".to_string());
    write.guest_module = Some("vmlinux".to_string());
    write.guest_symbol = Some("__x64_sys_write".to_string());
    let mut exec = base_exit("vm-a", Some("vm-a"), Some("0xabc"), "0x2408", false, true);
    exec.guest_os = write.guest_os.clone();
    exec.guest_process = write.guest_process.clone();
    exec.guest_thread = write.guest_thread.clone();
    exec.guest_module = write.guest_module.clone();
    exec.guest_symbol = write.guest_symbol.clone();

    assert!(wx.on_exit_event(&write).is_none());
    let alert = wx.on_exit_event(&exec).expect("W^X alert");

    assert_eq!(alert.guest_os.as_deref(), Some("linux"));
    assert_eq!(alert.guest_process.as_deref(), Some("sshd"));
    assert_eq!(alert.guest_thread.as_deref(), Some("sshd-worker"));
    assert_eq!(alert.guest_module.as_deref(), Some("vmlinux"));
    assert_eq!(alert.guest_symbol.as_deref(), Some("__x64_sys_write"));
}

#[test]
fn detector_cooldown_suppresses_duplicate_same_scope_page_and_reason() {
    let cfg = Config::default();
    let wx = WxEngine::new(&cfg);

    assert!(wx
        .on_exit_event(&base_exit_with_reason(
            "vm-a",
            Some("vm-a"),
            Some("0xabc"),
            "0x3000",
            true,
            false,
            Some("EPT_VIOLATION")
        ))
        .is_none());
    assert!(wx
        .on_exit_event(&base_exit_with_reason(
            "vm-a",
            Some("vm-a"),
            Some("0xabc"),
            "0x3008",
            false,
            true,
            Some("EPT_VIOLATION")
        ))
        .is_some());

    assert!(wx
        .on_exit_event(&base_exit_with_reason(
            "vm-a",
            Some("vm-a"),
            Some("0xabc"),
            "0x3000",
            true,
            false,
            Some("EPT_VIOLATION")
        ))
        .is_none());
    assert!(wx
        .on_exit_event(&base_exit_with_reason(
            "vm-a",
            Some("vm-a"),
            Some("0xabc"),
            "0x3008",
            false,
            true,
            Some("EPT_VIOLATION")
        ))
        .is_none());
    assert_eq!(wx.take_cooldown_suppressed_delta(), 1);
    assert_eq!(wx.take_cooldown_suppressed_delta(), 0);
}

#[test]
fn detector_cooldown_does_not_cross_address_space() {
    let cfg = Config::default();
    let wx = WxEngine::new(&cfg);

    assert!(wx
        .on_exit_event(&base_exit(
            "vm-a",
            Some("vm-a"),
            Some("0x111"),
            "0x4000",
            true,
            false
        ))
        .is_none());
    assert!(wx
        .on_exit_event(&base_exit(
            "vm-a",
            Some("vm-a"),
            Some("0x111"),
            "0x4008",
            false,
            true
        ))
        .is_some());

    assert!(wx
        .on_exit_event(&base_exit(
            "vm-a",
            Some("vm-a"),
            Some("0x222"),
            "0x4000",
            true,
            false
        ))
        .is_none());
    assert!(wx
        .on_exit_event(&base_exit(
            "vm-a",
            Some("vm-a"),
            Some("0x222"),
            "0x4008",
            false,
            true
        ))
        .is_some());
    assert_eq!(wx.take_cooldown_suppressed_delta(), 0);
}

#[test]
fn detector_cooldown_does_not_cross_source_reason() {
    let cfg = Config::default();
    let wx = WxEngine::new(&cfg);

    assert!(wx
        .on_exit_event(&base_exit_with_reason(
            "vm-a",
            Some("vm-a"),
            Some("0xabc"),
            "0x5000",
            true,
            false,
            Some("EPT_VIOLATION")
        ))
        .is_none());
    assert!(wx
        .on_exit_event(&base_exit_with_reason(
            "vm-a",
            Some("vm-a"),
            Some("0xabc"),
            "0x5008",
            false,
            true,
            Some("EPT_VIOLATION")
        ))
        .is_some());

    assert!(wx
        .on_exit_event(&base_exit_with_reason(
            "vm-a",
            Some("vm-a"),
            Some("0xabc"),
            "0x5000",
            true,
            false,
            Some("NPF")
        ))
        .is_none());
    assert!(wx
        .on_exit_event(&base_exit_with_reason(
            "vm-a",
            Some("vm-a"),
            Some("0xabc"),
            "0x5008",
            false,
            true,
            Some("NPF")
        ))
        .is_some());
    assert_eq!(wx.take_cooldown_suppressed_delta(), 0);
}

#[test]
fn zero_detector_cooldown_allows_duplicate_alerts() {
    let mut cfg = Config::default();
    cfg.general.wx_cooldown_ms = 0;
    let wx = WxEngine::new(&cfg);

    assert!(wx
        .on_exit_event(&base_exit(
            "vm-a",
            Some("vm-a"),
            Some("0xabc"),
            "0x6000",
            true,
            false
        ))
        .is_none());
    assert!(wx
        .on_exit_event(&base_exit(
            "vm-a",
            Some("vm-a"),
            Some("0xabc"),
            "0x6008",
            false,
            true
        ))
        .is_some());
    assert!(wx
        .on_exit_event(&base_exit(
            "vm-a",
            Some("vm-a"),
            Some("0xabc"),
            "0x6000",
            true,
            false
        ))
        .is_none());
    assert!(wx
        .on_exit_event(&base_exit(
            "vm-a",
            Some("vm-a"),
            Some("0xabc"),
            "0x6008",
            false,
            true
        ))
        .is_some());
    assert_eq!(wx.take_cooldown_suppressed_delta(), 0);
}
