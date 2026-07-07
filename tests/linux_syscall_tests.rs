use aegishv::linux_syscall::{
    inspect_linux_lstar, inspect_linux_syscall_table, linux_syscall_path_report,
};
use aegishv::linux_vmi::{LinuxTextRange, SyntheticLinuxVirtualMemory};
use aegishv::vmi::VmiErrorKind;
use aegishv::vmi_linux_profile::parse_linux_profile;
use aegishv::vmi_registers::{DescriptorTableRegister, X86_64RegisterSnapshot};

const SYS_CALL_TABLE: u64 = 0xffff_ffff_8120_0000;
const ENTRY_SYSCALL: u64 = 0xffff_ffff_8100_1000;
const READ_HANDLER: u64 = 0xffff_ffff_8100_2000;
const WRITE_HANDLER: u64 = 0xffff_ffff_8100_2100;
const OUTSIDE_TEXT: u64 = 0xffff_8880_dead_0000;

fn profile() -> aegishv::vmi_linux_profile::LinuxProfile {
    parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=fixed
symbol=_stext,0xffffffff81000000
symbol=_etext,0xffffffff81004000
symbol=entry_SYSCALL_64,0xffffffff81001000,0x40
symbol=sys_call_table,0xffffffff81200000
syscall=0,read,__x64_sys_read
syscall=1,write,__x64_sys_write
"#,
    )
    .expect("parse profile")
}

fn memory(read_handler: u64, write_handler: u64) -> SyntheticLinuxVirtualMemory {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    let mut table = vec![0u8; 16];
    table[0..8].copy_from_slice(&read_handler.to_le_bytes());
    table[8..16].copy_from_slice(&write_handler.to_le_bytes());
    memory
        .map_range(SYS_CALL_TABLE, table)
        .expect("map syscall table");
    memory
}

fn ranges() -> Vec<LinuxTextRange> {
    vec![LinuxTextRange {
        owner: "vmlinux".to_string(),
        start: 0xffff_ffff_8100_0000,
        end: 0xffff_ffff_8100_4000,
    }]
}

fn regs(lstar: u64) -> X86_64RegisterSnapshot {
    X86_64RegisterSnapshot::new(
        0x8005_0033,
        0,
        0x1000,
        0,
        1 << 11,
        DescriptorTableRegister::new(0, 0),
        DescriptorTableRegister::new(0, 0),
    )
    .with_lstar(lstar)
}

#[test]
fn syscall_table_accepts_handlers_inside_known_text_ranges() {
    let table = inspect_linux_syscall_table(
        &profile(),
        &memory(READ_HANDLER, WRITE_HANDLER),
        0,
        &ranges(),
    )
    .expect("inspect syscall table");

    assert!(table.ok);
    assert_eq!(table.table_address, SYS_CALL_TABLE);
    assert_eq!(table.entries.len(), 2);
    assert_eq!(table.entries[0].name, "read");
    assert_eq!(table.entries[0].owner.as_deref(), Some("vmlinux"));
    assert!(table.findings.is_empty());
}

#[test]
fn syscall_table_reports_handler_outside_kernel_and_module_text() {
    let table = inspect_linux_syscall_table(
        &profile(),
        &memory(READ_HANDLER, OUTSIDE_TEXT),
        0,
        &ranges(),
    )
    .expect("inspect syscall table");

    assert!(!table.ok);
    assert_eq!(table.entries[1].handler, OUTSIDE_TEXT);
    assert!(table.findings[0].contains("outside executable"));
}

#[test]
fn lstar_check_accepts_entry_syscall_range() {
    let report = inspect_linux_lstar(&profile(), &regs(ENTRY_SYSCALL + 0x10), 0, &ranges())
        .expect("inspect lstar");

    assert!(report.ok);
    assert_eq!(report.lstar, ENTRY_SYSCALL + 0x10);
    assert!(report.findings.is_empty());
}

#[test]
fn lstar_check_reports_target_outside_expected_entry_text() {
    let report =
        inspect_linux_lstar(&profile(), &regs(READ_HANDLER), 0, &ranges()).expect("inspect lstar");

    assert!(!report.ok);
    assert!(report.findings[0].contains("outside entry_SYSCALL_64"));
}

#[test]
fn lstar_check_requires_lstar_register() {
    let regs = X86_64RegisterSnapshot::partial();
    let err = inspect_linux_lstar(&profile(), &regs, 0, &ranges()).expect_err("missing lstar");

    assert_eq!(err.kind(), VmiErrorKind::InvalidInput);
    assert!(err.to_string().contains("lstar"));
}

#[test]
fn syscall_path_report_combines_lstar_and_table_findings() {
    let table = inspect_linux_syscall_table(
        &profile(),
        &memory(READ_HANDLER, OUTSIDE_TEXT),
        0,
        &ranges(),
    )
    .expect("table");
    let lstar = inspect_linux_lstar(&profile(), &regs(OUTSIDE_TEXT), 0, &ranges()).expect("lstar");
    let report = linux_syscall_path_report(&profile(), &table, &lstar);

    assert!(!report.ok);
    assert_eq!(report.os, "linux");
    assert_eq!(report.entry, Some(OUTSIDE_TEXT));
    assert_eq!(report.table, Some(SYS_CALL_TABLE));
    assert!(report.findings.len() >= 3);
}
