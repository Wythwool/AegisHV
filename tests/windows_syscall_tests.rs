use aegishv::vmi::VmiErrorKind;
use aegishv::vmi_registers::{DescriptorTableRegister, X86_64RegisterSnapshot};
use aegishv::windows_profile::parse_windows_profile;
use aegishv::windows_syscall::{
    inspect_windows_lstar, inspect_windows_ssdt, windows_syscall_path_report,
};
use aegishv::windows_vmi::{SyntheticWindowsVirtualMemory, WindowsTextRange};

const NT_BASE: u64 = 0xffff_f800_0000_0000;
const SERVICE_DESCRIPTOR: u64 = NT_BASE + 0x2000;
const SERVICE_TABLE: u64 = NT_BASE + 0x9000;
const KI_SYSTEM_CALL: u64 = NT_BASE + 0x6000;
const NT_ACCEPT: u64 = NT_BASE + 0x7000;
const NT_WRITE: u64 = NT_BASE + 0x7100;
const OUTSIDE_TEXT: u64 = NT_BASE + 0x20_0000;

fn profile(extra: &str) -> aegishv::windows_profile::WindowsProfile {
    parse_windows_profile(&format!(
        r#"
aegishv-windows-profile-v1
os=windows
arch=x86_64
build=10.0.22631.3155
variant=synthetic
pdb_file=ntkrnlmp.pdb
pdb_guid=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
pdb_age=2
symbol=ntoskrnl.exe,0x0,0x10000
symbol=KeServiceDescriptorTable,0x2000,0x20
symbol=KiSystemCall64,0x6000,0x80
symbol=NtAcceptConnectPort,0x7000,0x40
symbol=NtWriteFile,0x7100,0x40
syscall=0,NtAcceptConnectPort,NtAcceptConnectPort
syscall=1,NtWriteFile,NtWriteFile
{extra}
"#
    ))
    .expect("parse profile")
}

fn memory(
    first_handler: u64,
    second_handler: u64,
    service_count: u32,
) -> SyntheticWindowsVirtualMemory {
    let mut memory = SyntheticWindowsVirtualMemory::new();
    let mut descriptor = vec![0u8; 0x20];
    descriptor[0..8].copy_from_slice(&SERVICE_TABLE.to_le_bytes());
    descriptor[0x10..0x14].copy_from_slice(&service_count.to_le_bytes());
    memory
        .map_range(SERVICE_DESCRIPTOR, descriptor)
        .expect("map service descriptor");

    let mut table = vec![0u8; 8];
    table[0..4].copy_from_slice(&encode_ssdt_offset(first_handler).to_le_bytes());
    table[4..8].copy_from_slice(&encode_ssdt_offset(second_handler).to_le_bytes());
    memory.map_range(SERVICE_TABLE, table).expect("map SSDT");
    memory
}

fn ranges() -> Vec<WindowsTextRange> {
    vec![WindowsTextRange {
        owner: "ntoskrnl.exe".to_string(),
        start: NT_BASE,
        end: NT_BASE + 0x10000,
    }]
}

fn regs(lstar: u64) -> X86_64RegisterSnapshot {
    X86_64RegisterSnapshot::new(
        0,
        0,
        0x1000,
        0,
        0,
        DescriptorTableRegister::new(0, 0),
        DescriptorTableRegister::new(0, 0),
    )
    .with_lstar(lstar)
}

fn encode_ssdt_offset(handler: u64) -> i32 {
    let relative = i128::from(handler) - i128::from(SERVICE_TABLE);
    assert!(relative >= i128::from(i32::MIN >> 4));
    assert!(relative <= i128::from(i32::MAX >> 4));
    (relative as i32) << 4
}

#[test]
fn ssdt_accepts_handlers_inside_known_text_ranges() {
    let report = inspect_windows_ssdt(
        &profile(""),
        &memory(NT_ACCEPT, NT_WRITE, 2),
        NT_BASE,
        &ranges(),
    )
    .expect("inspect SSDT");

    assert!(report.ok);
    assert_eq!(report.descriptor_address, SERVICE_DESCRIPTOR);
    assert_eq!(report.table_address, SERVICE_TABLE);
    assert_eq!(report.service_count, 2);
    assert_eq!(report.entries.len(), 2);
    assert_eq!(report.entries[0].owner.as_deref(), Some("ntoskrnl.exe"));
    assert!(report.findings.is_empty());
}

#[test]
fn ssdt_reports_handlers_outside_executable_ranges() {
    let report = inspect_windows_ssdt(
        &profile(""),
        &memory(NT_ACCEPT, OUTSIDE_TEXT, 2),
        NT_BASE,
        &ranges(),
    )
    .expect("inspect SSDT");

    assert!(!report.ok);
    assert_eq!(report.entries[1].handler, OUTSIDE_TEXT);
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.contains("outside executable Windows ranges")));
}

#[test]
fn ssdt_reports_profile_syscall_outside_service_count() {
    let report = inspect_windows_ssdt(
        &profile(""),
        &memory(NT_ACCEPT, NT_WRITE, 1),
        NT_BASE,
        &ranges(),
    )
    .expect("inspect SSDT");

    assert!(!report.ok);
    assert_eq!(report.entries.len(), 1);
    assert!(report.findings[0].contains("outside SSDT service count"));
}

#[test]
fn lstar_check_accepts_ki_system_call_range() {
    let report = inspect_windows_lstar(
        &profile(""),
        &regs(KI_SYSTEM_CALL + 0x10),
        NT_BASE,
        &ranges(),
    )
    .expect("inspect LSTAR");

    assert!(report.ok);
    assert_eq!(report.lstar, KI_SYSTEM_CALL + 0x10);
    assert!(report.findings.is_empty());
}

#[test]
fn lstar_check_reports_target_outside_expected_entry_text() {
    let report = inspect_windows_lstar(&profile(""), &regs(NT_ACCEPT), NT_BASE, &ranges())
        .expect("inspect LSTAR");

    assert!(!report.ok);
    assert!(report.findings[0].contains("outside KiSystemCall64"));
}

#[test]
fn lstar_check_requires_lstar_register() {
    let regs = X86_64RegisterSnapshot::partial();
    let err =
        inspect_windows_lstar(&profile(""), &regs, NT_BASE, &ranges()).expect_err("missing LSTAR");

    assert_eq!(err.kind(), VmiErrorKind::InvalidInput);
    assert!(err.to_string().contains("lstar"));
}

#[test]
fn syscall_path_report_combines_lstar_and_ssdt_findings() {
    let ssdt = inspect_windows_ssdt(
        &profile(""),
        &memory(NT_ACCEPT, OUTSIDE_TEXT, 2),
        NT_BASE,
        &ranges(),
    )
    .expect("SSDT");
    let lstar = inspect_windows_lstar(&profile(""), &regs(OUTSIDE_TEXT), NT_BASE, &ranges())
        .expect("LSTAR");
    let report = windows_syscall_path_report(&profile(""), &ssdt, &lstar);

    assert!(!report.ok);
    assert_eq!(report.os, "windows");
    assert_eq!(report.entry, Some(OUTSIDE_TEXT));
    assert_eq!(report.table, Some(SERVICE_TABLE));
    assert!(report.findings.len() >= 3);
}
