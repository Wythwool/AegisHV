use aegishv::event::{Category, IdentityConfidence, Severity};
use aegishv::identity::{
    IdentityCacheResult, IdentityConflictReason, IdentityEnrichment, VmInventorySnapshot,
};
use aegishv::metrics::{ComponentStatus, IdentityQmpRefusalReason, Metrics, TraceInputReason};

#[test]
fn encodes_prometheus_text() {
    let m = Metrics::new().unwrap();
    m.inc_event(Category::Wx, Severity::Critical);
    m.inc_parse_ok();
    m.inc_wx_cooldown_suppressed(2);
    m.set_queue_depth(2, 8);
    m.set_wx_pages_tracked(5);
    let text = m.encode();
    assert!(text.contains("aegishv_events_total"));
    assert!(text.contains("aegishv_queue_capacity"));
    assert!(text.contains("aegishv_wx_cooldown_suppressed_total"));
    assert!(text.contains("aegishv_wx_pages_tracked"));
    assert!(text.contains("aegishv_spool_events_total"));
    assert!(text.contains("aegishv_syslog_write_failures_total"));
    assert!(text.contains("aegishv_journald_write_failures_total"));
    assert!(text.contains("aegishv_readiness_status"));
    assert!(text.contains("aegishv_component_status"));
    assert!(text.contains("aegishv_vmi_memory_read_attempts_total"));
    assert!(text.contains("aegishv_vmi_translation_attempts_total"));
}

#[test]
fn queue_depth_tracks_send_receive_and_full_drop() {
    let m = Metrics::new().unwrap();
    m.set_queue_depth(0, 2);

    m.record_queue_send_attempt(2);
    m.record_queue_send_attempt(2);
    assert_eq!(m.queue_depth(), 2);
    assert_eq!(m.queue_capacity(), 2);

    m.record_queue_send_attempt(2);
    m.record_queue_send_rejected();
    m.record_queue_full(2);
    assert_eq!(m.queue_depth(), 2);

    m.record_queue_receive();
    assert_eq!(m.queue_depth(), 1);
    m.record_queue_receive();
    assert_eq!(m.queue_depth(), 0);
    m.record_queue_receive();
    assert_eq!(m.queue_depth(), 0);
}

#[test]
fn queue_depth_encoding_uses_tracked_depth() {
    let m = Metrics::new().unwrap();
    m.set_queue_depth(0, 4);
    m.record_queue_send_attempt(4);
    m.record_queue_send_attempt(4);

    let text = m.encode();
    assert!(text.contains("aegishv_queue_depth 2.000000"));
    assert!(text.contains("aegishv_queue_capacity 4.000000"));
    assert!(text.contains("aegishv_queue_utilization 0.500000"));
}

#[test]
fn spool_counters_encode_preserved_and_dropped_events() {
    let m = Metrics::new().unwrap();
    m.inc_spool_event();
    m.inc_spool_write_failure();
    m.inc_spool_dropped();

    let text = m.encode();
    assert!(text.contains("aegishv_spool_events_total 1"));
    assert!(text.contains("aegishv_spool_write_failures_total 1"));
    assert!(text.contains("aegishv_spool_dropped_total 1"));
}

#[test]
fn syslog_failure_counter_uses_no_dynamic_labels() {
    let m = Metrics::new().unwrap();
    m.inc_syslog_write_failure();

    let text = m.encode();
    assert!(text.contains("aegishv_syslog_write_failures_total 1"));
    assert!(!text.contains("syslog_write_failures_total{"));
}

#[test]
fn journald_failure_counter_uses_no_dynamic_labels() {
    let m = Metrics::new().unwrap();
    m.inc_journald_write_failure();

    let text = m.encode();
    assert!(text.contains("aegishv_journald_write_failures_total 1"));
    assert!(!text.contains("journald_write_failures_total{"));
}

#[test]
fn trace_input_reason_counters_use_bounded_labels() {
    let m = Metrics::new().unwrap();
    m.inc_trace_input(TraceInputReason::Parsed);
    m.inc_trace_input(TraceInputReason::UnrelatedTracepoint);
    m.inc_trace_input(TraceInputReason::UnsupportedLine);
    m.inc_trace_input(TraceInputReason::MalformedKvmExit);
    m.inc_trace_input(TraceInputReason::ParserDegraded);
    m.inc_trace_input(TraceInputReason::ParserBug);

    let text = m.encode();

    for reason in [
        "parsed",
        "unrelated_tracepoint",
        "unsupported_line",
        "malformed_kvm_exit",
        "parser_degraded",
        "parser_bug",
    ] {
        assert!(text.contains(&format!(
            "aegishv_trace_inputs_total{{reason=\"{reason}\"}} 1"
        )));
    }
    assert!(!text.contains("qemu-system"));
    assert!(!text.contains("/sys/kernel"));
}

#[test]
fn identity_metrics_use_bounded_labels() {
    let m = Metrics::new().unwrap();
    let enrichment = IdentityEnrichment {
        cache_result: Some(IdentityCacheResult::Hit),
        confidence: Some(IdentityConfidence::High),
        ambiguous: false,
        conflict_reason: Some(IdentityConflictReason::PidReuse),
        conflict_event: None,
    };
    m.record_identity_enrichment(&enrichment);
    m.record_identity_enrichment(&IdentityEnrichment {
        cache_result: Some(IdentityCacheResult::Refusal),
        confidence: Some(IdentityConfidence::Low),
        ambiguous: true,
        conflict_reason: Some(IdentityConflictReason::QmpSocketMismatch),
        conflict_event: None,
    });
    let mut inventory = VmInventorySnapshot::empty();
    inventory.vm_count = 2;
    inventory.degraded = true;
    m.set_identity_inventory(&inventory);
    m.inc_identity_qmp_safety_refusal(IdentityQmpRefusalReason::AmbiguousIdentity);

    let text = m.encode();

    assert!(text.contains("aegishv_identity_cache_lookups_total{result=\"hit\"} 1"));
    assert!(text.contains("aegishv_identity_cache_lookups_total{result=\"miss\"} 0"));
    assert!(text.contains("aegishv_identity_cache_lookups_total{result=\"refusal\"} 1"));
    assert!(text
        .contains("aegishv_identity_enrichments_total{confidence=\"high\",ambiguous=\"false\"} 1"));
    assert!(text
        .contains("aegishv_identity_enrichments_total{confidence=\"low\",ambiguous=\"true\"} 1"));
    assert!(text.contains("aegishv_identity_conflicts_total{reason=\"pid_reuse\"} 1"));
    assert!(text.contains("aegishv_identity_conflicts_total{reason=\"qmp_socket_mismatch\"} 1"));
    assert!(text.contains("aegishv_identity_inventory_vms 2.000000"));
    assert!(text.contains("aegishv_identity_inventory_degraded 1.000000"));
    assert!(text
        .contains("aegishv_identity_qmp_safety_refusals_total{reason=\"ambiguous_identity\"} 1"));
    assert!(!text.contains("00000000-1111-2222-3333-444444444444"));
    assert!(!text.contains("/run/libvirt"));
    assert!(!text.contains("qemu-system"));
}

fn mark_ready_baseline(m: &Metrics) {
    m.mark_runtime_running();
    m.mark_collector_running();
    m.mark_metrics_listener_disabled();
    m.mark_output_ok();
    m.mark_policy_ok();
    m.mark_pmu_disabled();
    m.set_queue_depth(0, 4);
    m.mark_actions_ok();
}

#[test]
fn health_snapshot_distinguishes_startup_from_readiness() {
    let m = Metrics::new().unwrap();

    let snapshot = m.health_snapshot();

    assert_eq!(snapshot.status, "starting");
    assert!(snapshot.healthy);
    assert!(!snapshot.ready);
    assert_eq!(snapshot.components.runtime, ComponentStatus::Starting);
}

#[test]
fn health_snapshot_is_ready_when_required_components_are_ok() {
    let m = Metrics::new().unwrap();
    mark_ready_baseline(&m);

    let snapshot = m.health_snapshot();

    assert_eq!(snapshot.status, "ok");
    assert!(snapshot.healthy);
    assert!(snapshot.ready);
    assert_eq!(
        snapshot.components.metrics_listener,
        ComponentStatus::Disabled
    );
}

#[test]
fn degraded_output_keeps_health_but_removes_readiness() {
    let m = Metrics::new().unwrap();
    mark_ready_baseline(&m);

    m.mark_output_degraded();
    let snapshot = m.health_snapshot();

    assert_eq!(snapshot.status, "degraded");
    assert!(snapshot.healthy);
    assert!(!snapshot.ready);
    assert_eq!(snapshot.components.output, ComponentStatus::Degraded);
}

#[test]
fn failed_collector_removes_health_and_readiness() {
    let m = Metrics::new().unwrap();
    mark_ready_baseline(&m);

    m.mark_collector_failed();
    let snapshot = m.health_snapshot();

    assert_eq!(snapshot.status, "failed");
    assert!(!snapshot.healthy);
    assert!(!snapshot.ready);
    assert_eq!(snapshot.components.collector, ComponentStatus::Failed);
}

#[test]
fn action_failures_degrade_readiness_without_claiming_backend_health() {
    let m = Metrics::new().unwrap();
    mark_ready_baseline(&m);

    m.inc_policy_action("pause_vm", false);
    let snapshot = m.health_snapshot();

    assert_eq!(snapshot.status, "degraded");
    assert!(snapshot.healthy);
    assert!(!snapshot.ready);
    assert_eq!(snapshot.components.actions, ComponentStatus::Degraded);
}
