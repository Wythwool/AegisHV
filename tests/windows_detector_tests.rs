use std::collections::BTreeMap;

use aegishv::linux_integrity::sha256_hex;
use aegishv::vmi::VmiErrorKind;
use aegishv::vmi_registers::{DescriptorTableRegister, X86_64RegisterSnapshot};
use aegishv::windows_callbacks::WindowsCallbackWalkLimits;
use aegishv::windows_detectors::{run_windows_detector_checks, WindowsDetectorInputs};
use aegishv::windows_profile::parse_windows_profile;
use aegishv::windows_vmi::{SyntheticWindowsVirtualMemory, WindowsTextRange};

const NT_BASE: u64 = 0xffff_f800_0000_0000;
const SERVICE_DESCRIPTOR: u64 = NT_BASE + 0x1200;
const SERVICE_TABLE: u64 = NT_BASE + 0x1300;
const CALLBACK_TABLE: u64 = NT_BASE + 0x1400;
const IDT_BASE: u64 = 0xffff_8880_0000_8000;
const GDT_BASE: u64 = 0xffff_8880_0000_9000;
const CALLBACK_BLOCK: u64 = 0xffff_8880_0000_a000;
const DRIVER_BASE: u64 = 0xffff_f800_0100_0000;
const KI_SYSTEM_CALL: u64 = NT_BASE + 0x100;
const NT_ACCEPT: u64 = NT_BASE + 0x180;
const NT_WRITE: u64 = NT_BASE + 0x190;
const IDT_HANDLER: u64 = NT_BASE + 0x200;
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
symbol=ntoskrnl.exe,0x0,0x400
symbol=KeServiceDescriptorTable,0x1200,0x20
symbol=KiSystemCall64,0x100,0x40
symbol=NtAcceptConnectPort,0x180,0x10
symbol=NtWriteFile,0x190,0x10
symbol=PspCreateProcessNotifyRoutine,0x1400,0x40
offset=EX_CALLBACK_ROUTINE_BLOCK,Function,0x8,0x8
syscall=0,NtAcceptConnectPort,NtAcceptConnectPort
syscall=1,NtWriteFile,NtWriteFile
limit=vbs,not_present,synthetic profile does not enable VBS
{extra}
"#
    ))
    .expect("parse profile")
}

fn clean_memory() -> SyntheticWindowsVirtualMemory {
    memory(NT_WRITE, DRIVER_BASE + 0x2)
}

fn memory(second_syscall: u64, callback_target: u64) -> SyntheticWindowsVirtualMemory {
    let mut memory = SyntheticWindowsVirtualMemory::new();
    let kernel_text = vec![0x90u8; 0x400];
    memory
        .map_range(NT_BASE, kernel_text)
        .expect("map kernel text");
    memory
        .map_range(DRIVER_BASE, b"driver-text".to_vec())
        .expect("map driver text");

    let mut descriptor = vec![0u8; 0x20];
    descriptor[0..8].copy_from_slice(&SERVICE_TABLE.to_le_bytes());
    descriptor[0x10..0x14].copy_from_slice(&2u32.to_le_bytes());
    memory
        .map_range(SERVICE_DESCRIPTOR, descriptor)
        .expect("map service descriptor");
    let mut table = vec![0u8; 8];
    table[0..4].copy_from_slice(&encode_ssdt_offset(NT_ACCEPT).to_le_bytes());
    table[4..8].copy_from_slice(&encode_ssdt_offset(second_syscall).to_le_bytes());
    memory.map_range(SERVICE_TABLE, table).expect("map SSDT");

    map_u64(&mut memory, CALLBACK_TABLE, CALLBACK_BLOCK | 0x7);
    map_u64(&mut memory, CALLBACK_BLOCK + 0x8, callback_target);
    memory
        .map_range(IDT_BASE + 14 * 16, gate_bytes(IDT_HANDLER, true))
        .expect("map IDT gate");
    memory
        .map_range(GDT_BASE + 0x10, code_descriptor_bytes(true))
        .expect("map GDT descriptor");
    memory
}

fn ranges() -> Vec<WindowsTextRange> {
    vec![
        WindowsTextRange {
            owner: "ntoskrnl.exe".to_string(),
            start: NT_BASE,
            end: NT_BASE + 0x400,
        },
        WindowsTextRange {
            owner: "win32k.sys".to_string(),
            start: DRIVER_BASE,
            end: DRIVER_BASE + 11,
        },
    ]
}

fn regs(lstar: u64) -> X86_64RegisterSnapshot {
    X86_64RegisterSnapshot::new(
        0,
        0,
        0x1000,
        0,
        0,
        DescriptorTableRegister::new(IDT_BASE, 0x0fff),
        DescriptorTableRegister::new(GDT_BASE, 0x00ff),
    )
    .with_lstar(lstar)
}

fn baselines(kernel: &[u8], driver: &[u8]) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
    let mut kernel_baselines = BTreeMap::new();
    kernel_baselines.insert("ntoskrnl.exe".to_string(), sha256_hex(kernel));
    let mut driver_baselines = BTreeMap::new();
    driver_baselines.insert("win32k.sys".to_string(), sha256_hex(driver));
    (kernel_baselines, driver_baselines)
}

fn inputs<'a>(
    profile: &'a aegishv::windows_profile::WindowsProfile,
    memory: &'a SyntheticWindowsVirtualMemory,
    registers: &'a X86_64RegisterSnapshot,
    ranges: &'a [WindowsTextRange],
    kernel_baselines: &'a BTreeMap<String, String>,
    driver_baselines: &'a BTreeMap<String, String>,
) -> WindowsDetectorInputs<'a> {
    WindowsDetectorInputs {
        profile,
        memory,
        registers,
        nt_base: NT_BASE,
        executable_ranges: ranges,
        kernel_text_baselines: kernel_baselines,
        driver_text_baselines: driver_baselines,
        process_callback_slots: 1,
        callback_limits: WindowsCallbackWalkLimits::default(),
        critical_idt_vectors: &[14],
        gdt_selectors: &[0x10],
    }
}

fn encode_ssdt_offset(handler: u64) -> i32 {
    let relative = i128::from(handler) - i128::from(SERVICE_TABLE);
    assert!(relative >= i128::from(i32::MIN >> 4));
    assert!(relative <= i128::from(i32::MAX >> 4));
    (relative as i32) << 4
}

fn map_u64(memory: &mut SyntheticWindowsVirtualMemory, address: u64, value: u64) {
    memory
        .map_range(address, value.to_le_bytes())
        .expect("map u64");
}

fn gate_bytes(offset: u64, present: bool) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    bytes[0..2].copy_from_slice(&(offset as u16).to_le_bytes());
    bytes[2..4].copy_from_slice(&0x10u16.to_le_bytes());
    bytes[5] = if present { 0x8e } else { 0x0e };
    bytes[6..8].copy_from_slice(&((offset >> 16) as u16).to_le_bytes());
    bytes[8..12].copy_from_slice(&((offset >> 32) as u32).to_le_bytes());
    bytes
}

fn code_descriptor_bytes(present: bool) -> [u8; 8] {
    let mut bytes = [0u8; 8];
    bytes[0..2].copy_from_slice(&0xffffu16.to_le_bytes());
    bytes[5] = if present { 0x9a } else { 0x1a };
    bytes[6] = 0x20;
    bytes
}

#[test]
fn windows_detector_runner_accepts_clean_synthetic_state() {
    let profile = profile("");
    let memory = clean_memory();
    let ranges = ranges();
    let regs = regs(KI_SYSTEM_CALL + 0x8);
    let (kernel_baselines, driver_baselines) = baselines(&vec![0x90u8; 0x400], b"driver-text");

    let run = run_windows_detector_checks(inputs(
        &profile,
        &memory,
        &regs,
        &ranges,
        &kernel_baselines,
        &driver_baselines,
    ))
    .expect("run detectors");

    assert!(run.ok, "{:?}", run.findings);
    assert!(run.findings.is_empty());
    assert!(run.checks.iter().all(|check| check.ok));
    assert!(run.syscall_path.ok);
    assert_eq!(run.callbacks.callbacks.len(), 1);
}

#[test]
fn windows_detector_runner_combines_findings_without_hiding_context() {
    let profile = profile("limit=hvci,degraded,HVCI state is metadata-only in this fixture\n");
    let memory = memory(OUTSIDE_TEXT, OUTSIDE_TEXT);
    let ranges = ranges();
    let regs = regs(OUTSIDE_TEXT);
    let (kernel_baselines, driver_baselines) = baselines(b"changed", b"driver-text");

    let run = run_windows_detector_checks(inputs(
        &profile,
        &memory,
        &regs,
        &ranges,
        &kernel_baselines,
        &driver_baselines,
    ))
    .expect("run detectors");

    assert!(!run.ok);
    assert!(run.findings.iter().any(|finding| finding.check == "lstar"));
    assert!(run.findings.iter().any(|finding| finding.check == "ssdt"));
    assert!(run
        .findings
        .iter()
        .any(|finding| finding.check == "callbacks"));
    assert!(run
        .findings
        .iter()
        .any(|finding| finding.check == "kernel_text_hash"));
    assert!(run
        .findings
        .iter()
        .any(|finding| finding.check == "protection_limits"));
}

#[test]
fn windows_detector_runner_returns_typed_error_for_unsupported_profile() {
    let profile = parse_windows_profile(
        r#"
aegishv-windows-profile-v1
os=windows
arch=x86_64
build=10.0.22631.3155
pdb_file=ntkrnlmp.pdb
pdb_guid=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
pdb_age=2
symbol=ntoskrnl.exe,0x0,0x400
symbol=KiSystemCall64,0x100,0x40
symbol=KeServiceDescriptorTable,0x1200,0x20
"#,
    )
    .expect("parse profile");
    let memory = clean_memory();
    let ranges = ranges();
    let regs = regs(KI_SYSTEM_CALL);
    let (kernel_baselines, driver_baselines) = baselines(&vec![0x90u8; 0x400], b"driver-text");

    let err = run_windows_detector_checks(inputs(
        &profile,
        &memory,
        &regs,
        &ranges,
        &kernel_baselines,
        &driver_baselines,
    ))
    .expect_err("missing syscall profile must fail");

    assert_eq!(err.kind(), VmiErrorKind::MissingProfile);
    assert!(err.to_string().contains("syscall entries"));
}
