use std::collections::BTreeMap;

use aegishv::linux_integrity::{
    check_linux_kernel_text_hash, check_linux_module_text_hashes, check_text_hashes, sha256_hex,
    LinuxTextHashStatus,
};
use aegishv::linux_vmi::{LinuxTextRange, SyntheticLinuxVirtualMemory};
use aegishv::vmi::VmiErrorKind;

const KERNEL_TEXT: u64 = 0xffff_ffff_8100_0000;
const MODULE_TEXT: u64 = 0xffff_ffff_c001_0000;

fn memory() -> SyntheticLinuxVirtualMemory {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    memory
        .map_range(KERNEL_TEXT, b"kernel text bytes".to_vec())
        .expect("map kernel text");
    memory
        .map_range(MODULE_TEXT, b"module text bytes".to_vec())
        .expect("map module text");
    memory
}

fn ranges() -> Vec<LinuxTextRange> {
    vec![
        LinuxTextRange {
            owner: "vmlinux".to_string(),
            start: KERNEL_TEXT,
            end: KERNEL_TEXT + 17,
        },
        LinuxTextRange {
            owner: "kvm".to_string(),
            start: MODULE_TEXT,
            end: MODULE_TEXT + 17,
        },
    ]
}

#[test]
fn sha256_matches_standard_empty_and_abc_vectors() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn kernel_text_hash_matches_known_baseline() {
    let mut baselines = BTreeMap::new();
    baselines.insert("vmlinux".to_string(), sha256_hex(b"kernel text bytes"));

    let report =
        check_linux_kernel_text_hash(&memory(), &ranges(), &baselines).expect("check kernel hash");

    assert!(report.ok);
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, LinuxTextHashStatus::Match);
}

#[test]
fn missing_kernel_baseline_is_not_clean() {
    let report =
        check_linux_kernel_text_hash(&memory(), &ranges(), &BTreeMap::new()).expect("check hash");

    assert!(!report.ok);
    assert_eq!(
        report.results[0].status,
        LinuxTextHashStatus::UnknownBaseline
    );
    assert!(report.findings[0].contains("unknown_baseline"));
}

#[test]
fn module_text_hash_distinguishes_unknown_from_mismatch() {
    let mut baselines = BTreeMap::new();
    baselines.insert("kvm".to_string(), sha256_hex(b"different module"));

    let report =
        check_linux_module_text_hashes(&memory(), &ranges(), &baselines).expect("check modules");

    assert!(!report.ok);
    assert_eq!(report.results[0].owner, "kvm");
    assert_eq!(report.results[0].status, LinuxTextHashStatus::Mismatch);
    assert!(report.findings[0].contains("mismatch"));
}

#[test]
fn text_hash_rejects_empty_or_inverted_ranges() {
    let err = check_text_hashes(
        &memory(),
        &[LinuxTextRange {
            owner: "bad".to_string(),
            start: 0x2000,
            end: 0x2000,
        }],
        &BTreeMap::new(),
    )
    .expect_err("empty range must fail");

    assert_eq!(err.kind(), VmiErrorKind::InconsistentSnapshot);
    assert!(err.to_string().contains("empty or inverted"));
}
