use aegishv::linux_vmi::{LinuxTextRange, SyntheticLinuxVirtualMemory};
use aegishv::linux_x86::{
    decode_gdt_descriptor, decode_idt_gate, inspect_linux_control_registers, inspect_linux_gdt,
    inspect_linux_idt, LinuxControlPolicy, X86_CR0_WP, X86_CR4_SMAP, X86_CR4_SMEP, X86_EFER_NXE,
};
use aegishv::vmi::VmiErrorKind;
use aegishv::vmi_registers::{DescriptorTableRegister, X86_64RegisterSnapshot};

const IDT_BASE: u64 = 0xffff_8880_0000_8000;
const GDT_BASE: u64 = 0xffff_8880_0000_9000;
const PAGE_FAULT_HANDLER: u64 = 0xffff_ffff_8100_3000;
const OUTSIDE_TEXT: u64 = 0xffff_8880_dead_0000;

fn regs() -> X86_64RegisterSnapshot {
    X86_64RegisterSnapshot::new(
        X86_CR0_WP,
        0,
        0x1000,
        X86_CR4_SMEP | X86_CR4_SMAP,
        X86_EFER_NXE,
        DescriptorTableRegister::new(IDT_BASE, 0x0fff),
        DescriptorTableRegister::new(GDT_BASE, 0x00ff),
    )
}

fn text_ranges() -> Vec<LinuxTextRange> {
    vec![LinuxTextRange {
        owner: "vmlinux".to_string(),
        start: 0xffff_ffff_8100_0000,
        end: 0xffff_ffff_8100_5000,
    }]
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
fn control_register_check_accepts_strict_kernel_bits() {
    let report = inspect_linux_control_registers(&regs(), LinuxControlPolicy::strict_x86_64())
        .expect("inspect controls");

    assert!(report.ok);
    assert!(report.findings.is_empty());
}

#[test]
fn control_register_check_reports_missing_policy_bits() {
    let bad = X86_64RegisterSnapshot::new(
        0,
        0,
        0x1000,
        X86_CR4_SMEP,
        0,
        DescriptorTableRegister::new(IDT_BASE, 0x0fff),
        DescriptorTableRegister::new(GDT_BASE, 0x00ff),
    );

    let report = inspect_linux_control_registers(&bad, LinuxControlPolicy::strict_x86_64())
        .expect("inspect controls");

    assert!(!report.ok);
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.contains("CR0.WP")));
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.contains("CR4.SMAP")));
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.contains("EFER.NXE")));
}

#[test]
fn idt_inspection_accepts_present_gate_inside_kernel_text() {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    memory
        .map_range(IDT_BASE + 14 * 16, gate_bytes(PAGE_FAULT_HANDLER, true))
        .expect("map idt gate");

    let report = inspect_linux_idt(&memory, &regs(), &text_ranges(), &[14]).expect("inspect idt");

    assert!(report.ok);
    assert_eq!(report.idt_gates[0].vector, 14);
    assert_eq!(report.idt_gates[0].offset, PAGE_FAULT_HANDLER);
}

#[test]
fn idt_inspection_reports_gate_outside_text_ranges() {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    memory
        .map_range(IDT_BASE + 14 * 16, gate_bytes(OUTSIDE_TEXT, true))
        .expect("map idt gate");

    let report = inspect_linux_idt(&memory, &regs(), &text_ranges(), &[14]).expect("inspect idt");

    assert!(!report.ok);
    assert!(report.findings[0].contains("outside executable"));
}

#[test]
fn gdt_inspection_decodes_present_code_descriptor() {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    memory
        .map_range(GDT_BASE + 0x10, code_descriptor_bytes(true))
        .expect("map gdt descriptor");

    let report = inspect_linux_gdt(&memory, &regs(), &[0x10]).expect("inspect gdt");

    assert!(report.ok);
    assert_eq!(report.gdt_descriptors[0].selector, 0x10);
    assert!(report.gdt_descriptors[0].present);
    assert!(report.gdt_descriptors[0].executable);
    assert!(report.gdt_descriptors[0].long_mode);
}

#[test]
fn gdt_inspection_reports_not_present_descriptor() {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    memory
        .map_range(GDT_BASE + 0x10, code_descriptor_bytes(false))
        .expect("map gdt descriptor");

    let report = inspect_linux_gdt(&memory, &regs(), &[0x10]).expect("inspect gdt");

    assert!(!report.ok);
    assert!(report.findings[0].contains("not present"));
}

#[test]
fn descriptor_bounds_are_checked_against_table_limits() {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    memory
        .map_range(IDT_BASE, gate_bytes(PAGE_FAULT_HANDLER, true))
        .expect("map idt gate");
    let tight = X86_64RegisterSnapshot::new(
        X86_CR0_WP,
        0,
        0x1000,
        X86_CR4_SMEP | X86_CR4_SMAP,
        X86_EFER_NXE,
        DescriptorTableRegister::new(IDT_BASE, 0x000f),
        DescriptorTableRegister::new(GDT_BASE, 0x0007),
    );

    let err = inspect_linux_idt(&memory, &tight, &text_ranges(), &[14])
        .expect_err("out-of-limit IDT vector must fail");

    assert_eq!(err.kind(), VmiErrorKind::InconsistentSnapshot);
    assert!(err.to_string().contains("outside IDTR limit"));
}

#[test]
fn raw_descriptor_decoders_preserve_offsets_and_access_bits() {
    let gate = decode_idt_gate(0x80, gate_bytes(PAGE_FAULT_HANDLER, true));
    let segment = decode_gdt_descriptor(0x10, code_descriptor_bytes(true));

    assert_eq!(gate.offset, PAGE_FAULT_HANDLER);
    assert!(gate.present);
    assert_eq!(segment.access, 0x9a);
    assert!(segment.readable_or_writable);
}
