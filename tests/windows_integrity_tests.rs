use std::collections::BTreeMap;

use aegishv::linux_integrity::sha256_hex;
use aegishv::vmi::VmiErrorKind;
use aegishv::windows_integrity::{
    check_windows_driver_text_hashes, check_windows_kernel_text_hash, check_windows_text_hashes,
    WindowsTextHashStatus,
};
use aegishv::windows_vmi::{SyntheticWindowsVirtualMemory, WindowsTextRange};

const KERNEL_TEXT: u64 = 0xffff_f800_0000_0000;
const DRIVER_TEXT: u64 = 0xffff_f800_0100_0000;

fn memory() -> SyntheticWindowsVirtualMemory {
    let mut memory = SyntheticWindowsVirtualMemory::new();
    memory
        .map_range(KERNEL_TEXT, b"windows kernel text".to_vec())
        .expect("map kernel text");
    memory
        .map_range(DRIVER_TEXT, b"windows driver text".to_vec())
        .expect("map driver text");
    memory
}

fn ranges() -> Vec<WindowsTextRange> {
    vec![
        WindowsTextRange {
            owner: "ntoskrnl.exe".to_string(),
            start: KERNEL_TEXT,
            end: KERNEL_TEXT + 19,
        },
        WindowsTextRange {
            owner: "win32k.sys".to_string(),
            start: DRIVER_TEXT,
            end: DRIVER_TEXT + 19,
        },
    ]
}

#[test]
fn kernel_text_hash_matches_known_baseline() {
    let mut baselines = BTreeMap::new();
    baselines.insert(
        "ntoskrnl.exe".to_string(),
        sha256_hex(b"windows kernel text"),
    );

    let report =
        check_windows_kernel_text_hash(&memory(), &ranges(), &baselines).expect("check hash");

    assert!(report.ok);
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, WindowsTextHashStatus::Match);
}

#[test]
fn missing_kernel_baseline_is_not_clean() {
    let report =
        check_windows_kernel_text_hash(&memory(), &ranges(), &BTreeMap::new()).expect("check hash");

    assert!(!report.ok);
    assert_eq!(
        report.results[0].status,
        WindowsTextHashStatus::UnknownBaseline
    );
    assert!(report.findings[0].contains("unknown_baseline"));
}

#[test]
fn driver_text_hash_distinguishes_unknown_from_mismatch() {
    let mut baselines = BTreeMap::new();
    baselines.insert("win32k.sys".to_string(), sha256_hex(b"different driver"));

    let report =
        check_windows_driver_text_hashes(&memory(), &ranges(), &baselines).expect("check drivers");

    assert!(!report.ok);
    assert_eq!(report.results[0].owner, "win32k.sys");
    assert_eq!(report.results[0].status, WindowsTextHashStatus::Mismatch);
    assert!(report.findings[0].contains("mismatch"));
}

#[test]
fn text_hash_rejects_empty_or_inverted_ranges() {
    let err = check_windows_text_hashes(
        &memory(),
        &[WindowsTextRange {
            owner: "bad.sys".to_string(),
            start: 0x2000,
            end: 0x2000,
        }],
        &BTreeMap::new(),
    )
    .expect_err("empty range must fail");

    assert_eq!(err.kind(), VmiErrorKind::InconsistentSnapshot);
    assert!(err.to_string().contains("empty or inverted"));
}
