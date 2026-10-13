use aegishv::vmi::VmiErrorKind;
use aegishv::vmi_registers::{DescriptorTableRegister, X86_64RegisterSnapshot};
use aegishv::windows_vmi::{SyntheticWindowsVirtualMemory, WindowsTextRange};
use aegishv::windows_x64::{
    decode_windows_gdt_descriptor, decode_windows_idt_gate, inspect_windows_gdt,
    inspect_windows_idt,
};

const IDT_BASE: u64 = 0xffff_8880_0000_8000;
const GDT_BASE: u64 = 0xffff_8880_0000_9000;
const PAGE_FAULT_HANDLER: u64 = 0xffff_f800_0000_3000;
const OUTSIDE_TEXT: u64 = 0xffff_f800_0020_0000;

fn regs() -> X86_64RegisterSnapshot {
    X86_64RegisterSnapshot::new(
        0,
        0,
        0x1000,
        0,
        0,
        DescriptorTableRegister::new(IDT_BASE, 0x0fff),
        DescriptorTableRegister::new(GDT_BASE, 0x00ff),
    )
}

fn text_ranges() -> Vec<WindowsTextRange> {
    vec![WindowsTextRange {
        owner: "ntoskrnl.exe".to_string(),
        start: 0xffff_f800_0000_0000,
        end: 0xffff_f800_0001_0000,
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
fn idt_inspection_accepts_present_gate_inside_kernel_text() {
    let mut memory = SyntheticWindowsVirtualMemory::new();
    memory
        .map_range(IDT_BASE + 14 * 16, gate_bytes(PAGE_FAULT_HANDLER, true))
        .expect("map IDT gate");

    let report = inspect_windows_idt(&memory, &regs(), &text_ranges(), &[14]).expect("inspect IDT");

    assert!(report.ok);
    assert_eq!(report.idt_gates[0].vector, 14);
    assert_eq!(report.idt_gates[0].offset, PAGE_FAULT_HANDLER);
}

#[test]
fn idt_inspection_reports_gate_outside_text_ranges() {
    let mut memory = SyntheticWindowsVirtualMemory::new();
    memory
        .map_range(IDT_BASE + 14 * 16, gate_bytes(OUTSIDE_TEXT, true))
        .expect("map IDT gate");

    let report = inspect_windows_idt(&memory, &regs(), &text_ranges(), &[14]).expect("inspect IDT");

    assert!(!report.ok);
    assert!(report.findings[0].contains("outside executable Windows ranges"));
}

#[test]
fn gdt_inspection_decodes_present_code_descriptor() {
    let mut memory = SyntheticWindowsVirtualMemory::new();
    memory
        .map_range(GDT_BASE + 0x10, code_descriptor_bytes(true))
        .expect("map GDT descriptor");

    let report = inspect_windows_gdt(&memory, &regs(), &[0x10]).expect("inspect GDT");

    assert!(report.ok);
    assert_eq!(report.gdt_descriptors[0].selector, 0x10);
    assert!(report.gdt_descriptors[0].present);
    assert!(report.gdt_descriptors[0].executable);
    assert!(report.gdt_descriptors[0].long_mode);
}

#[test]
fn gdt_inspection_reports_not_present_descriptor() {
    let mut memory = SyntheticWindowsVirtualMemory::new();
    memory
        .map_range(GDT_BASE + 0x10, code_descriptor_bytes(false))
        .expect("map GDT descriptor");

    let report = inspect_windows_gdt(&memory, &regs(), &[0x10]).expect("inspect GDT");

    assert!(!report.ok);
    assert!(report.findings[0].contains("not present"));
}

#[test]
fn descriptor_bounds_are_checked_against_table_limits() {
    let mut memory = SyntheticWindowsVirtualMemory::new();
    memory
        .map_range(IDT_BASE, gate_bytes(PAGE_FAULT_HANDLER, true))
        .expect("map IDT gate");
    let tight = X86_64RegisterSnapshot::new(
        0,
        0,
        0x1000,
        0,
        0,
        DescriptorTableRegister::new(IDT_BASE, 0x000f),
        DescriptorTableRegister::new(GDT_BASE, 0x0007),
    );

    let err = inspect_windows_idt(&memory, &tight, &text_ranges(), &[14])
        .expect_err("out-of-limit IDT vector must fail");

    assert_eq!(err.kind(), VmiErrorKind::InconsistentSnapshot);
    assert!(err.to_string().contains("outside IDTR limit"));
}

#[test]
fn raw_descriptor_decoders_preserve_offsets_and_access_bits() {
    let gate = decode_windows_idt_gate(0x80, gate_bytes(PAGE_FAULT_HANDLER, true));
    let segment = decode_windows_gdt_descriptor(0x10, code_descriptor_bytes(true));

    assert_eq!(gate.offset, PAGE_FAULT_HANDLER);
    assert!(gate.present);
    assert_eq!(segment.access, 0x9a);
    assert!(segment.readable_or_writable);
}
