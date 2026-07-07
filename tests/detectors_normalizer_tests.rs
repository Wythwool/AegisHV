use aegishv::detectors::kernel_text::{linux_kernel_text_findings, offline_source};
use aegishv::detectors::syscall_hooks::{
    linux_syscall_hook_findings, windows_syscall_hook_findings,
};
use aegishv::detectors::wx::wx_detection_from_event;
use aegishv::detectors::{ConfidenceLevel, DetectionKind, ProfileConfidence};
use aegishv::event::{AddrInfo, Category, Event, Severity, WxInfo};
use aegishv::linux_integrity::{LinuxIntegrityReport, LinuxTextHashResult, LinuxTextHashStatus};
use aegishv::linux_syscall::{LinuxLstarReport, LinuxSyscallTableReport};
use aegishv::windows_syscall::{WindowsLstarReport, WindowsSsdtReport};

fn source() -> aegishv::detectors::DetectionSource {
    offline_source("offline-vmi", ProfileConfidence::ExactBuild)
}

#[test]
fn kernel_text_normalizer_reports_hash_mismatch_and_ignores_clean_ranges() {
    let report = LinuxIntegrityReport {
        ok: false,
        results: vec![
            LinuxTextHashResult {
                owner: "vmlinux".to_string(),
                start: 0x1000,
                end: 0x2000,
                sha256: "bad".to_string(),
                expected_sha256: Some("good".to_string()),
                status: LinuxTextHashStatus::Mismatch,
            },
            LinuxTextHashResult {
                owner: "kvm".to_string(),
                start: 0x3000,
                end: 0x4000,
                sha256: "same".to_string(),
                expected_sha256: Some("same".to_string()),
                status: LinuxTextHashStatus::Match,
            },
        ],
        findings: vec!["text range 'vmlinux' hash status is mismatch".to_string()],
    };

    let records = linux_kernel_text_findings(&report, source(), Some("vm-a")).expect("normalize");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, DetectionKind::KernelTextTamper);
    assert_eq!(records[0].vm_id.as_deref(), Some("vm-a"));
    assert_eq!(records[0].entity.as_deref(), Some("vmlinux"));
}

#[test]
fn kernel_text_normalizer_reports_empty_report_as_unsupported() {
    let report = LinuxIntegrityReport {
        ok: true,
        results: Vec::new(),
        findings: Vec::new(),
    };

    let err = linux_kernel_text_findings(&report, source(), None).expect_err("empty report");

    assert_eq!(err.kind(), aegishv::vmi::VmiErrorKind::Unsupported);
    assert!(err.to_string().contains("no hash results"));
}

#[test]
fn syscall_hook_normalizer_dedupes_lstar_and_table_findings() {
    let table = LinuxSyscallTableReport {
        ok: false,
        table_address: 0x1000,
        entries: vec![aegishv::linux_syscall::LinuxSyscallEntry {
            number: 0,
            name: "read".to_string(),
            expected_symbol: Some("__x64_sys_read".to_string()),
            handler: 0x2000,
            owner: None,
        }],
        findings: vec!["handler outside text".to_string()],
    };
    let lstar = LinuxLstarReport {
        ok: false,
        lstar: 0x3000,
        expected_symbol: "entry_SYSCALL_64".to_string(),
        findings: vec![
            "handler outside text".to_string(),
            "lstar outside text".to_string(),
        ],
    };

    let records = linux_syscall_hook_findings(&table, &lstar, source(), Some("vm-a"))
        .expect("normalize syscall findings");

    assert_eq!(records.len(), 2);
    assert!(records
        .iter()
        .all(|record| record.kind == DetectionKind::SyscallHook));
}

#[test]
fn windows_syscall_hook_normalizer_keeps_ssdt_service_count_findings() {
    let ssdt = WindowsSsdtReport {
        ok: false,
        descriptor_address: 0x1000,
        table_address: 0x2000,
        service_count: 1,
        entries: Vec::new(),
        findings: vec!["syscall 8 is outside SSDT service count 1".to_string()],
    };
    let lstar = WindowsLstarReport {
        ok: true,
        lstar: 0x3000,
        expected_symbol: "KiSystemCall64".to_string(),
        findings: Vec::new(),
    };

    let records = windows_syscall_hook_findings(&ssdt, &lstar, source(), None)
        .expect("normalize windows syscall findings");

    assert_eq!(records.len(), 1);
    assert!(records[0].detail.contains("SSDT service count"));
}

#[test]
fn wx_detection_bridge_preserves_guest_process_and_page_range() {
    let mut ev = Event::base(
        Category::Wx,
        Severity::Critical,
        "2026-07-07T00:00:00Z".to_string(),
        "vm-a".to_string(),
    );
    ev.vm_id = Some("libvirt:11111111-1111-1111-1111-111111111111".to_string());
    ev.guest_process = Some("powershell.exe".to_string());
    ev.guest_symbol = Some("JITCode".to_string());
    ev.addr = Some(AddrInfo {
        rip: None,
        gva: None,
        gpa: Some("0x4000".to_string()),
        qual: None,
    });
    ev.wx = Some(WxInfo {
        writer_rip: Some("0x1000".to_string()),
        executor_rip: Some("0x2000".to_string()),
        delta_ms: 42,
        page_size: Some(4096),
        confidence: 0.9,
    });

    let record = wx_detection_from_event(&ev)
        .expect("bridge W^X")
        .expect("W^X detection");

    assert_eq!(record.kind, DetectionKind::WxCorrelation);
    assert_eq!(record.entity.as_deref(), Some("powershell.exe"));
    assert_eq!(record.symbol.as_deref(), Some("JITCode"));
    assert_eq!(record.range_start, Some(0x4000));
    assert_eq!(record.range_end, Some(0x5000));
    assert_eq!(record.confidence.level, ConfidenceLevel::High);
}

#[test]
fn wx_detection_bridge_rejects_wx_event_without_payload() {
    let ev = Event::base(
        Category::Wx,
        Severity::Critical,
        "2026-07-07T00:00:00Z".to_string(),
        "vm-a".to_string(),
    );

    let err = wx_detection_from_event(&ev).expect_err("missing wx payload");

    assert!(err.to_string().contains("missing wx payload"));
}
