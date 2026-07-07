use aegishv::linux_hooks::{inspect_linux_ftrace_ops, inspect_linux_kprobes, LinuxHookWalkLimits};
use aegishv::linux_vmi::{LinuxTextRange, SyntheticLinuxVirtualMemory};
use aegishv::vmi::VmiErrorKind;
use aegishv::vmi_linux_profile::parse_linux_profile;

const FTRACE_HEAD: u64 = 0xffff_8880_0000_1000;
const FTRACE_OP_A: u64 = 0xffff_8880_0000_1100;
const FTRACE_OP_B: u64 = 0xffff_8880_0000_1200;
const KPROBE_TABLE: u64 = 0xffff_8880_0000_2000;
const KPROBE_A: u64 = 0xffff_8880_0000_2200;
const TEXT_START: u64 = 0xffff_ffff_8100_0000;
const TEXT_END: u64 = 0xffff_ffff_8100_5000;
const MODULE_START: u64 = 0xffff_ffff_c001_0000;
const MODULE_END: u64 = 0xffff_ffff_c001_3000;
const OUTSIDE_TEXT: u64 = 0xffff_8880_7000_0000;

fn profile(extra: &str) -> aegishv::vmi_linux_profile::LinuxProfile {
    parse_linux_profile(&format!(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=fixed
symbol=ftrace_ops_list,{FTRACE_HEAD:#x}
symbol=kprobe_table,{KPROBE_TABLE:#x}
offset=ftrace_ops,list,0x0,0x10
offset=ftrace_ops,func,0x10,0x8
offset=ftrace_ops,flags,0x18,0x4
offset=kprobe,hlist,0x0,0x10
offset=kprobe,addr,0x10,0x8
offset=kprobe,pre_handler,0x18,0x8
offset=kprobe,post_handler,0x20,0x8
offset=kprobe,fault_handler,0x28,0x8
{extra}
"#
    ))
    .expect("parse profile")
}

fn ranges() -> Vec<LinuxTextRange> {
    vec![
        LinuxTextRange {
            owner: "vmlinux".to_string(),
            start: TEXT_START,
            end: TEXT_END,
        },
        LinuxTextRange {
            owner: "module:kvm".to_string(),
            start: MODULE_START,
            end: MODULE_END,
        },
    ]
}

fn map_u64(memory: &mut SyntheticLinuxVirtualMemory, address: u64, value: u64) {
    memory
        .map_range(address, value.to_le_bytes())
        .expect("map u64");
}

fn map_u32(memory: &mut SyntheticLinuxVirtualMemory, address: u64, value: u32) {
    memory
        .map_range(address, value.to_le_bytes())
        .expect("map u32");
}

#[test]
fn ftrace_inspection_accepts_callbacks_inside_known_text() {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    map_u64(&mut memory, FTRACE_HEAD, FTRACE_OP_A);
    map_u64(&mut memory, FTRACE_OP_A, FTRACE_OP_B);
    map_u64(&mut memory, FTRACE_OP_A + 0x10, TEXT_START + 0x100);
    map_u32(&mut memory, FTRACE_OP_A + 0x18, 0x1);
    map_u64(&mut memory, FTRACE_OP_B, FTRACE_HEAD);
    map_u64(&mut memory, FTRACE_OP_B + 0x10, MODULE_START + 0x80);
    map_u32(&mut memory, FTRACE_OP_B + 0x18, 0x2);

    let report = inspect_linux_ftrace_ops(
        &profile(""),
        &memory,
        0,
        &ranges(),
        LinuxHookWalkLimits::default(),
    )
    .expect("inspect ftrace");

    assert!(report.ok);
    assert_eq!(report.operations.len(), 2);
    assert_eq!(
        report.operations[0].callback_owner.as_deref(),
        Some("vmlinux")
    );
    assert_eq!(report.operations[1].flags, Some(0x2));
}

#[test]
fn ftrace_inspection_reports_callbacks_outside_text() {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    map_u64(&mut memory, FTRACE_HEAD, FTRACE_OP_A);
    map_u64(&mut memory, FTRACE_OP_A, FTRACE_HEAD);
    map_u64(&mut memory, FTRACE_OP_A + 0x10, OUTSIDE_TEXT);
    map_u32(&mut memory, FTRACE_OP_A + 0x18, 0);

    let report = inspect_linux_ftrace_ops(
        &profile(""),
        &memory,
        0,
        &ranges(),
        LinuxHookWalkLimits::default(),
    )
    .expect("inspect ftrace");

    assert!(!report.ok);
    assert_eq!(report.operations[0].callback_owner, None);
    assert!(report.findings[0].contains("outside executable"));
}

#[test]
fn ftrace_inspection_is_unsupported_without_profile_offsets() {
    let missing = parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=fixed
symbol=ftrace_ops_list,0xffff888000001000
"#,
    )
    .expect("parse profile");
    let memory = SyntheticLinuxVirtualMemory::new();

    let err = inspect_linux_ftrace_ops(
        &missing,
        &memory,
        0,
        &ranges(),
        LinuxHookWalkLimits::default(),
    )
    .expect_err("missing offsets must be unsupported");

    assert_eq!(err.kind(), VmiErrorKind::Unsupported);
    assert!(err.to_string().contains("ftrace_ops.list"));
}

#[test]
fn kprobe_inspection_reports_handlers_outside_known_text() {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    map_u64(&mut memory, KPROBE_TABLE, KPROBE_A);
    map_u64(&mut memory, KPROBE_A, 0);
    map_u64(&mut memory, KPROBE_A + 0x10, TEXT_START + 0x250);
    map_u64(&mut memory, KPROBE_A + 0x18, OUTSIDE_TEXT);
    map_u64(&mut memory, KPROBE_A + 0x20, MODULE_START + 0x180);
    map_u64(&mut memory, KPROBE_A + 0x28, 0);

    let report = inspect_linux_kprobes(
        &profile(""),
        &memory,
        0,
        &ranges(),
        1,
        LinuxHookWalkLimits::default(),
    )
    .expect("inspect kprobes");

    assert!(!report.ok);
    assert_eq!(report.probes.len(), 1);
    assert_eq!(report.probes[0].target_owner.as_deref(), Some("vmlinux"));
    assert_eq!(report.probes[0].handlers.len(), 2);
    assert_eq!(report.probes[0].handlers[0].owner, None);
    assert!(report.findings[0].contains("pre_handler"));
}

#[test]
fn kprobe_inspection_rejects_invalid_bucket_count() {
    let memory = SyntheticLinuxVirtualMemory::new();

    let err = inspect_linux_kprobes(
        &profile(""),
        &memory,
        0,
        &ranges(),
        0,
        LinuxHookWalkLimits::default(),
    )
    .expect_err("zero buckets must fail");

    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("bucket count"));
}
