#![no_main]

use aegishv::actions::ActionDispatcher;
use aegishv::config::{Config, QmpMapping};
use aegishv::event::{IdentityConfidence, IdentityInfo};
use aegishv::identity::{
    IDENTITY_SOURCE_AMBIGUOUS, IDENTITY_SOURCE_FALLBACK_PID, IDENTITY_SOURCE_LIBVIRT_XML,
    IDENTITY_SOURCE_START_TIME_VERIFIED,
};
use aegishv::metrics::Metrics;
use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 4096;
const QMP_REFUSAL_CASES: u8 = 7;
const MISSING_IDENTITY_SELECTOR: u8 = 0;
const PID_ONLY_IDENTITY_SELECTOR: u8 = 1;
const AMBIGUOUS_IDENTITY_SELECTOR: u8 = 2;
const UNVERIFIED_IDENTITY_SELECTOR: u8 = 3;
const STALE_IDENTITY_SELECTOR: u8 = 4;
const CONFLICTING_IDENTITY_SELECTOR: u8 = 5;
const LOW_CONFIDENCE_SELECTOR: u8 = 6;

fn token(data: &[u8], fallback: &str) -> String {
    let mut out = String::new();
    for byte in data.iter().take(48) {
        let ch = match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b':' => char::from(*byte),
            _ => 'x',
        };
        out.push(ch);
    }
    if out.is_empty() {
        fallback.to_string()
    } else {
        out
    }
}

fn qmp_refusal_case(
    selector: u8,
) -> (
    &'static str,
    Option<IdentityInfo>,
    Vec<String>,
    Option<&'static str>,
) {
    match selector % QMP_REFUSAL_CASES {
        MISSING_IDENTITY_SELECTOR => (
            "missing_identity",
            None,
            Vec::new(),
            Some("libvirt:fuzz-vm"),
        ),
        PID_ONLY_IDENTITY_SELECTOR => (
            "pid_only_identity",
            Some(IdentityInfo {
                sources: vec![IDENTITY_SOURCE_FALLBACK_PID.to_string()],
                confidence: IdentityConfidence::Low,
                start_time_verified: false,
                ambiguous: false,
            }),
            Vec::new(),
            Some("libvirt:fuzz-vm"),
        ),
        AMBIGUOUS_IDENTITY_SELECTOR => (
            "ambiguous_identity",
            Some(IdentityInfo {
                sources: vec![IDENTITY_SOURCE_AMBIGUOUS.to_string()],
                confidence: IdentityConfidence::Low,
                start_time_verified: false,
                ambiguous: true,
            }),
            Vec::new(),
            Some("libvirt:fuzz-vm"),
        ),
        UNVERIFIED_IDENTITY_SELECTOR => (
            "unverified_identity",
            Some(IdentityInfo {
                sources: vec![IDENTITY_SOURCE_LIBVIRT_XML.to_string()],
                confidence: IdentityConfidence::High,
                start_time_verified: false,
                ambiguous: false,
            }),
            Vec::new(),
            Some("libvirt:fuzz-vm"),
        ),
        STALE_IDENTITY_SELECTOR => (
            "stale_identity",
            Some(IdentityInfo {
                sources: vec![
                    IDENTITY_SOURCE_LIBVIRT_XML.to_string(),
                    IDENTITY_SOURCE_START_TIME_VERIFIED.to_string(),
                ],
                confidence: IdentityConfidence::High,
                start_time_verified: true,
                ambiguous: false,
            }),
            vec!["identity_conflict:stale_cache".to_string()],
            Some("libvirt:fuzz-vm"),
        ),
        CONFLICTING_IDENTITY_SELECTOR => (
            "conflicting_identity",
            Some(IdentityInfo {
                sources: vec![
                    IDENTITY_SOURCE_LIBVIRT_XML.to_string(),
                    IDENTITY_SOURCE_START_TIME_VERIFIED.to_string(),
                ],
                confidence: IdentityConfidence::High,
                start_time_verified: true,
                ambiguous: false,
            }),
            vec!["identity_conflict:qmp_socket_mismatch".to_string()],
            Some("libvirt:fuzz-vm"),
        ),
        LOW_CONFIDENCE_SELECTOR => (
            "low_confidence",
            Some(IdentityInfo {
                sources: vec![
                    IDENTITY_SOURCE_LIBVIRT_XML.to_string(),
                    IDENTITY_SOURCE_START_TIME_VERIFIED.to_string(),
                ],
                confidence: IdentityConfidence::Medium,
                start_time_verified: true,
                ambiguous: false,
            }),
            Vec::new(),
            Some("libvirt:fuzz-vm"),
        ),
        _ => unreachable!("selector is reduced by QMP_REFUSAL_CASES"),
    }
}

fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(MAX_INPUT_BYTES)];
    let mut cfg = Config::default();
    cfg.actions.retries = 0;
    cfg.actions.qmp = vec![QmpMapping {
        vm_id: "libvirt:fuzz-vm".to_string(),
        vm: "fuzz-vm".to_string(),
        socket: "qmp-fuzz-socket".to_string(),
    }];

    let Ok(dispatcher) = ActionDispatcher::new(&cfg) else {
        return;
    };
    let Ok(metrics) = Metrics::new() else {
        return;
    };

    let selector = data.first().copied().unwrap_or(0);
    let (case_name, identity, tags, vm_id) = qmp_refusal_case(selector);
    let kind = match data.get(1).copied().unwrap_or(0) % 4 {
        0 => "pause_vm",
        1 => "resume_vm",
        2 => "dump_guest_memory",
        _ => "quarantine_nic",
    };
    let vm = token(data.get(2..).unwrap_or_default(), "fuzz-vm");
    let execute = data.get(3).map_or(true, |byte| byte & 1 == 0);

    let event = dispatcher.run_action(
        &metrics,
        Some(case_name),
        &vm,
        vm_id,
        kind,
        Some("fuzz-dump"),
        Some("fuzz-nic"),
        identity.as_ref(),
        &tags,
        execute,
    );
    let _ = event.to_json();
    let _ = metrics.encode();
});
