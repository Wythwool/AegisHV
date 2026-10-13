use std::collections::BTreeMap;

use aegishv::linux_detectors::{run_linux_detector_checks, LinuxDetectorInputs};
use aegishv::linux_integrity::sha256_hex;
use aegishv::linux_vmi::{LinuxTextRange, SyntheticLinuxVirtualMemory};
use aegishv::linux_x86::{
    LinuxControlPolicy, X86_CR0_WP, X86_CR4_SMAP, X86_CR4_SMEP, X86_EFER_NXE,
};
use aegishv::vmi::VmiErrorKind;
use aegishv::vmi_linux_profile::parse_linux_profile;
use aegishv::vmi_registers::{DescriptorTableRegister, X86_64RegisterSnapshot};

const SYS_CALL_TABLE: u64 = 0xffff_ffff_8120_0000;
const KERNEL_TEXT: u64 = 0xffff_ffff_8100_0000;
const ENTRY_SYSCALL: u64 = KERNEL_TEXT + 1;
const READ_HANDLER: u64 = KERNEL_TEXT + 3;
const WRITE_HANDLER: u64 = KERNEL_TEXT + 5;
const MODULE_TEXT: u64 = 0xffff_ffff_c001_0000;
const OUTSIDE_TEXT: u64 = 0xffff_8880_dead_0000;

fn profile(with_syscalls: bool) -> aegishv::vmi_linux_profile::LinuxProfile {
    let syscalls = if with_syscalls {
        "syscall=0,read,__x64_sys_read\nsyscall=1,write,__x64_sys_write"
    } else {
        ""
    };
    parse_linux_profile(&format!(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=fixed
symbol=_stext,{KERNEL_TEXT:#x}
symbol=_etext,{:#x}
symbol=entry_SYSCALL_64,{ENTRY_SYSCALL:#x},0x40
symbol=sys_call_table,{SYS_CALL_TABLE:#x}
{syscalls}
"#,
        KERNEL_TEXT + 0x1000
    ))
    .expect("parse profile")
}

fn memory(
    read_handler: u64,
    write_handler: u64,
    kernel_text: &[u8],
) -> SyntheticLinuxVirtualMemory {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    let mut table = vec![0u8; 16];
    table[0..8].copy_from_slice(&read_handler.to_le_bytes());
    table[8..16].copy_from_slice(&write_handler.to_le_bytes());
    memory
        .map_range(SYS_CALL_TABLE, table)
        .expect("map syscall table");
    memory
        .map_range(KERNEL_TEXT, kernel_text.to_vec())
        .expect("map kernel text");
    memory
        .map_range(MODULE_TEXT, b"module-text".to_vec())
        .expect("map module text");
    memory
}

fn ranges() -> Vec<LinuxTextRange> {
    vec![
        LinuxTextRange {
            owner: "vmlinux".to_string(),
            start: KERNEL_TEXT,
            end: KERNEL_TEXT + 11,
        },
        LinuxTextRange {
            owner: "kvm".to_string(),
            start: MODULE_TEXT,
            end: MODULE_TEXT + 11,
        },
    ]
}

fn regs(lstar: u64, strict_bits: bool) -> X86_64RegisterSnapshot {
    let cr0 = if strict_bits { X86_CR0_WP } else { 0 };
    let cr4 = if strict_bits {
        X86_CR4_SMEP | X86_CR4_SMAP
    } else {
        X86_CR4_SMEP
    };
    let efer = if strict_bits { X86_EFER_NXE } else { 0 };
    X86_64RegisterSnapshot::new(
        cr0,
        0,
        0x1000,
        cr4,
        efer,
        DescriptorTableRegister::new(0, 0),
        DescriptorTableRegister::new(0, 0),
    )
    .with_lstar(lstar)
}

fn baselines(kernel: &[u8], module: &[u8]) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
    let mut kernel_baselines = BTreeMap::new();
    kernel_baselines.insert("vmlinux".to_string(), sha256_hex(kernel));
    let mut module_baselines = BTreeMap::new();
    module_baselines.insert("kvm".to_string(), sha256_hex(module));
    (kernel_baselines, module_baselines)
}

#[test]
fn linux_detector_runner_accepts_clean_synthetic_state() {
    let profile = profile(true);
    let memory = memory(READ_HANDLER, WRITE_HANDLER, b"kernel-text");
    let ranges = ranges();
    let (kernel_baselines, module_baselines) = baselines(b"kernel-text", b"module-text");

    let run = run_linux_detector_checks(LinuxDetectorInputs {
        profile: &profile,
        memory: &memory,
        registers: &regs(ENTRY_SYSCALL + 0x2, true),
        slide: 0,
        executable_ranges: &ranges,
        kernel_text_baselines: &kernel_baselines,
        module_text_baselines: &module_baselines,
        control_policy: LinuxControlPolicy::strict_x86_64(),
    })
    .expect("run detectors");

    assert!(run.ok, "{:?}", run.findings);
    assert!(run.findings.is_empty());
    assert!(run.checks.iter().all(|check| check.ok));
    assert!(run.syscall_path.ok);
}

#[test]
fn linux_detector_runner_combines_findings_without_hiding_context() {
    let profile = profile(true);
    let memory = memory(READ_HANDLER, OUTSIDE_TEXT, b"kernel-text");
    let ranges = ranges();
    let (kernel_baselines, module_baselines) = baselines(b"changed", b"module-text");

    let run = run_linux_detector_checks(LinuxDetectorInputs {
        profile: &profile,
        memory: &memory,
        registers: &regs(OUTSIDE_TEXT, false),
        slide: 0,
        executable_ranges: &ranges,
        kernel_text_baselines: &kernel_baselines,
        module_text_baselines: &module_baselines,
        control_policy: LinuxControlPolicy::strict_x86_64(),
    })
    .expect("run detectors");

    assert!(!run.ok);
    assert!(run.findings.iter().any(|finding| finding.check == "lstar"));
    assert!(run
        .findings
        .iter()
        .any(|finding| finding.check == "syscall_table"));
    assert!(run
        .findings
        .iter()
        .any(|finding| finding.check == "control_registers"));
    assert!(run
        .findings
        .iter()
        .any(|finding| finding.check == "kernel_text_hash"));
}

#[test]
fn linux_detector_runner_returns_typed_error_for_unsupported_profile() {
    let profile = profile(false);
    let memory = memory(READ_HANDLER, WRITE_HANDLER, b"kernel-text");
    let ranges = ranges();
    let (kernel_baselines, module_baselines) = baselines(b"kernel-text", b"module-text");

    let err = run_linux_detector_checks(LinuxDetectorInputs {
        profile: &profile,
        memory: &memory,
        registers: &regs(ENTRY_SYSCALL, true),
        slide: 0,
        executable_ranges: &ranges,
        kernel_text_baselines: &kernel_baselines,
        module_text_baselines: &module_baselines,
        control_policy: LinuxControlPolicy::strict_x86_64(),
    })
    .expect_err("missing syscall profile must fail");

    assert_eq!(err.kind(), VmiErrorKind::MissingProfile);
    assert!(err.to_string().contains("syscall entries"));
}
