use aegishv::detectors::hidden_module::{detect_hidden_modules, ModuleInventory, ModuleKey};
use aegishv::detectors::hidden_process::{detect_hidden_processes, ProcessInventory, ProcessKey};
use aegishv::detectors::jit::{JitAllowRule, JitAllowlist};
use aegishv::detectors::memory::{
    detect_executable_anonymous_mappings, detect_rwx_mappings, MemoryMapping, MemoryRegionKind,
};
use aegishv::detectors::{DetectionKind, DetectionSource, ProfileConfidence, SourceReliability};
use aegishv::vmi::VmiErrorKind;

fn source(name: &str) -> DetectionSource {
    DetectionSource::new(
        name,
        SourceReliability::OfflineSnapshot,
        ProfileConfidence::ExactBuild,
    )
}

fn mapping(kind: MemoryRegionKind, writable: bool, executable: bool) -> MemoryMapping {
    MemoryMapping {
        vm_id: Some("vm-a".to_string()),
        process: Some("java".to_string()),
        module: None,
        start: 0x1000,
        end: 0x2000,
        readable: true,
        writable,
        executable,
        kind,
        source: source("offline-pages"),
    }
}

#[test]
fn hidden_process_detector_compares_memory_and_os_inventories() {
    let memory = ProcessInventory {
        source: source("memory-eprocess"),
        supported: true,
        processes: vec![
            ProcessKey {
                pid: 4,
                image: "System".to_string(),
            },
            ProcessKey {
                pid: 1200,
                image: "agent.exe".to_string(),
            },
        ],
    };
    let os = ProcessInventory {
        source: source("os-list"),
        supported: true,
        processes: vec![ProcessKey {
            pid: 4,
            image: "System".to_string(),
        }],
    };

    let records = detect_hidden_processes(&memory, &os, Some("vm-a")).expect("detect");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, DetectionKind::HiddenProcess);
    assert!(records[0].detail.contains("agent.exe"));
}

#[test]
fn hidden_process_detector_refuses_unsupported_inventory() {
    let unsupported = ProcessInventory {
        source: source("memory"),
        supported: false,
        processes: Vec::new(),
    };
    let os = ProcessInventory {
        source: source("os"),
        supported: true,
        processes: Vec::new(),
    };

    let err = detect_hidden_processes(&unsupported, &os, None).expect_err("unsupported");

    assert_eq!(err.kind(), VmiErrorKind::Unsupported);
}

#[test]
fn hidden_module_detector_reports_driver_missing_from_os_inventory() {
    let memory = ModuleInventory {
        source: source("memory-modules"),
        supported: true,
        modules: vec![ModuleKey {
            name: "stealth.sys".to_string(),
            base: 0xffff_f800_0100_0000,
        }],
    };
    let os = ModuleInventory {
        source: source("os-modules"),
        supported: true,
        modules: Vec::new(),
    };

    let records = detect_hidden_modules(&memory, &os, Some("vm-a")).expect("detect");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, DetectionKind::HiddenModule);
    assert_eq!(records[0].entity.as_deref(), Some("stealth.sys"));
}

#[test]
fn executable_anonymous_detector_honors_jit_allowlist() {
    let mapping = mapping(MemoryRegionKind::Anonymous, false, true);
    let blocked = detect_executable_anonymous_mappings(&[mapping.clone()], &JitAllowlist::empty())
        .expect("detect");
    let allowlist = JitAllowlist::new(vec![JitAllowRule::new(
        Some("^java$"),
        None,
        0x1000,
        0x2000,
    )
    .expect("allow rule")]);
    let allowed = detect_executable_anonymous_mappings(&[mapping], &allowlist).expect("detect");

    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].kind, DetectionKind::ExecutableAnonymousMemory);
    assert!(allowed.is_empty());
}

#[test]
fn rwx_detector_reports_read_write_execute_mapping() {
    let records = detect_rwx_mappings(&[mapping(MemoryRegionKind::Anonymous, true, true)])
        .expect("detect RWX");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, DetectionKind::RwxMapping);
}

#[test]
fn memory_detector_rejects_empty_ranges_and_bad_jit_rules() {
    let mut bad = mapping(MemoryRegionKind::Anonymous, true, true);
    bad.end = bad.start;

    let err = detect_rwx_mappings(&[bad]).expect_err("bad range");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);

    let err = JitAllowRule::new(Some("("), None, 1, 2).expect_err("bad pattern");
    assert!(err.to_string().contains("invalid JIT process pattern"));
}
