use std::fs;
use std::path::PathBuf;

use aegishv::detectors::dedupe::DetectionAggregator;
use aegishv::detectors::state::{
    load_detector_state, parse_detector_state, render_detector_state, save_detector_state,
    DetectorState,
};
use aegishv::detectors::{
    score_detection, AttributionQuality, DetectionKind, DetectionRecord, DetectionSource,
    ProfileConfidence, ScoreFactors, SourceReliability,
};
use aegishv::event::{IdentityConfidence, Severity};
use aegishv::incidents::{correlate_incidents, IncidentStatus};

fn source() -> DetectionSource {
    DetectionSource::new(
        "offline-test",
        SourceReliability::VerifiedSnapshot,
        ProfileConfidence::VerifiedSnapshot,
    )
}

fn record(kind: DetectionKind, detector: &str, vm_id: &str) -> DetectionRecord {
    let score = score_detection(ScoreFactors {
        base_severity: Severity::High,
        source: SourceReliability::VerifiedSnapshot,
        attribution: AttributionQuality::GuestSymbol,
        profile: ProfileConfidence::VerifiedSnapshot,
        identity: IdentityConfidence::High,
        data_loss: false,
        policy_match: false,
    });
    DetectionRecord::new(detector, kind, kind.as_str(), "detail", source(), score)
        .with_vm_id(vm_id)
        .with_entity("kernel")
        .with_range(0x1000, 0x2000)
}

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "aegishv-{name}-{}-{}.state",
        std::process::id(),
        unique_tick()
    ))
}

fn unique_tick() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos()
}

#[test]
fn dedupe_aggregator_keys_by_detector_vm_entity_range_and_symbol() {
    let mut aggregator = DetectionAggregator::new();
    let first = aggregator
        .observe(
            record(
                DetectionKind::KernelTextTamper,
                "kernel_text_tamper",
                "vm-a",
            ),
            100,
        )
        .expect("observe");
    let second = aggregator
        .observe(
            record(
                DetectionKind::KernelTextTamper,
                "kernel_text_tamper",
                "vm-a",
            ),
            150,
        )
        .expect("observe");

    assert_eq!(aggregator.len(), 1);
    assert_eq!(first.count, 1);
    assert_eq!(second.count, 2);
    assert_eq!(second.first_seen_ms, 100);
    assert_eq!(second.last_seen_ms, 150);
}

#[test]
fn incident_model_requires_wx_syscall_and_kernel_text_on_same_vm() {
    let mut aggregator = DetectionAggregator::new();
    for (kind, detector) in [
        (DetectionKind::WxCorrelation, "wx_correlation"),
        (DetectionKind::SyscallHook, "syscall_hook"),
        (DetectionKind::KernelTextTamper, "kernel_text_tamper"),
    ] {
        aggregator
            .observe(record(kind, detector, "vm-a"), 100)
            .expect("observe");
    }
    aggregator
        .observe(
            record(DetectionKind::WxCorrelation, "wx_correlation", "vm-b"),
            100,
        )
        .expect("observe");
    let entries = aggregator.entries().cloned().collect::<Vec<_>>();

    let incidents = correlate_incidents(&entries);

    assert_eq!(incidents.len(), 1);
    assert_eq!(incidents[0].vm_id, "vm-a");
    assert_eq!(incidents[0].status, IncidentStatus::Open);
    assert_eq!(incidents[0].kinds.len(), 3);
}

#[test]
fn detector_state_round_trips_dedupe_and_incident_records() {
    let mut aggregator = DetectionAggregator::new();
    for (idx, (kind, detector)) in [
        (DetectionKind::WxCorrelation, "wx_correlation"),
        (DetectionKind::SyscallHook, "syscall_hook"),
        (DetectionKind::KernelTextTamper, "kernel_text_tamper"),
    ]
    .into_iter()
    .enumerate()
    {
        aggregator
            .observe(record(kind, detector, "vm-a"), 100 + idx as u64)
            .expect("observe");
    }
    let entries = aggregator.entries().cloned().collect::<Vec<_>>();
    let incidents = correlate_incidents(&entries);
    let state = DetectorState::from_runtime(&entries, &incidents);
    let rendered = render_detector_state(&state);

    let parsed = parse_detector_state(&rendered).expect("parse state");

    assert_eq!(parsed.dedupe.len(), 3);
    assert_eq!(parsed.incidents.len(), 1);
    assert_eq!(parsed.incidents[0].vm_id, "vm-a");
}

#[test]
fn detector_state_load_emits_sensor_event_for_corrupt_state() {
    let path = temp_path("corrupt");
    fs::write(&path, "not-a-state\nbad").expect("write corrupt state");

    let loaded = load_detector_state(&path);

    assert!(loaded.state.dedupe.is_empty());
    assert_eq!(loaded.sensor_events.len(), 1);
    assert!(loaded.sensor_events[0].message.contains("ignored"));
    let _ = fs::remove_file(path);
}

#[test]
fn detector_state_save_uses_versioned_file() {
    let path = temp_path("save");
    let state = DetectorState::default();

    save_detector_state(&path, &state).expect("save state");
    let text = fs::read_to_string(&path).expect("read state");

    assert!(text.starts_with("aegishv-detector-state-v1"));
    let _ = fs::remove_file(path);
}
