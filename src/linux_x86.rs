use crate::linux_vmi::{
    address_in_text_ranges, LinuxTextRange, LinuxVirtualMemoryReader, LinuxVmiError,
};
use crate::vmi_registers::X86_64RegisterSnapshot;

pub const X86_CR0_WP: u64 = 1 << 16;
pub const X86_CR4_SMEP: u64 = 1 << 20;
pub const X86_CR4_SMAP: u64 = 1 << 21;
pub const X86_EFER_NXE: u64 = 1 << 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinuxControlPolicy {
    pub require_cr0_wp: bool,
    pub require_cr4_smep: bool,
    pub require_cr4_smap: bool,
    pub require_efer_nxe: bool,
}

impl Default for LinuxControlPolicy {
    fn default() -> Self {
        Self {
            require_cr0_wp: true,
            require_cr4_smep: false,
            require_cr4_smap: false,
            require_efer_nxe: true,
        }
    }
}

impl LinuxControlPolicy {
    pub fn strict_x86_64() -> Self {
        Self {
            require_cr0_wp: true,
            require_cr4_smep: true,
            require_cr4_smap: true,
            require_efer_nxe: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxControlReport {
    pub ok: bool,
    pub findings: Vec<String>,
}

pub fn inspect_linux_control_registers(
    regs: &X86_64RegisterSnapshot,
    policy: LinuxControlPolicy,
) -> Result<LinuxControlReport, LinuxVmiError> {
    let cr0 = regs.cr0()?;
    let cr4 = regs.cr4()?;
    let efer = regs.efer()?;
    let mut findings = Vec::new();

    if policy.require_cr0_wp && cr0 & X86_CR0_WP == 0 {
        findings.push("CR0.WP is clear while policy requires write protect".to_string());
    }
    if policy.require_cr4_smep && cr4 & X86_CR4_SMEP == 0 {
        findings.push("CR4.SMEP is clear while policy requires SMEP".to_string());
    }
    if policy.require_cr4_smap && cr4 & X86_CR4_SMAP == 0 {
        findings.push("CR4.SMAP is clear while policy requires SMAP".to_string());
    }
    if policy.require_efer_nxe && efer & X86_EFER_NXE == 0 {
        findings.push("EFER.NXE is clear while policy requires NX".to_string());
    }

    Ok(LinuxControlReport {
        ok: findings.is_empty(),
        findings,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86IdtGate {
    pub vector: u8,
    pub offset: u64,
    pub selector: u16,
    pub type_attr: u8,
    pub present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86SegmentDescriptor {
    pub selector: u16,
    pub base: u64,
    pub limit: u32,
    pub access: u8,
    pub flags: u8,
    pub present: bool,
    pub executable: bool,
    pub readable_or_writable: bool,
    pub long_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxDescriptorReport {
    pub ok: bool,
    pub findings: Vec<String>,
    pub idt_gates: Vec<X86IdtGate>,
    pub gdt_descriptors: Vec<X86SegmentDescriptor>,
}

pub fn inspect_linux_idt(
    memory: &dyn LinuxVirtualMemoryReader,
    regs: &X86_64RegisterSnapshot,
    executable_ranges: &[LinuxTextRange],
    critical_vectors: &[u8],
) -> Result<LinuxDescriptorReport, LinuxVmiError> {
    let idtr = regs.idtr()?;
    let mut gates = Vec::new();
    let mut findings = Vec::new();

    for vector in critical_vectors {
        let offset = u64::from(*vector) * 16;
        if offset + 15 > u64::from(idtr.limit) {
            return Err(LinuxVmiError::InconsistentSnapshot {
                detail: format!("IDT vector {vector} is outside IDTR limit {}", idtr.limit),
            });
        }
        let address =
            idtr.base
                .checked_add(offset)
                .ok_or_else(|| LinuxVmiError::InconsistentSnapshot {
                    detail: "IDT descriptor address overflowed".to_string(),
                })?;
        let gate = read_idt_gate(memory, address, *vector)?;
        if !gate.present {
            findings.push(format!("IDT vector {vector} is not present"));
        } else if address_in_text_ranges(gate.offset, executable_ranges).is_none() {
            findings.push(format!(
                "IDT vector {vector} handler 0x{:x} is outside executable kernel/module ranges",
                gate.offset
            ));
        }
        gates.push(gate);
    }

    Ok(LinuxDescriptorReport {
        ok: findings.is_empty(),
        findings,
        idt_gates: gates,
        gdt_descriptors: Vec::new(),
    })
}

pub fn inspect_linux_gdt(
    memory: &dyn LinuxVirtualMemoryReader,
    regs: &X86_64RegisterSnapshot,
    selectors: &[u16],
) -> Result<LinuxDescriptorReport, LinuxVmiError> {
    let gdtr = regs.gdtr()?;
    let mut descriptors = Vec::new();
    let mut findings = Vec::new();

    for selector in selectors {
        let index_offset = u64::from(selector & !0x7);
        if index_offset == 0 {
            continue;
        }
        if index_offset + 7 > u64::from(gdtr.limit) {
            return Err(LinuxVmiError::InconsistentSnapshot {
                detail: format!(
                    "GDT selector 0x{selector:x} is outside GDTR limit {}",
                    gdtr.limit
                ),
            });
        }
        let address = gdtr.base.checked_add(index_offset).ok_or_else(|| {
            LinuxVmiError::InconsistentSnapshot {
                detail: "GDT descriptor address overflowed".to_string(),
            }
        })?;
        let descriptor = read_gdt_descriptor(memory, address, *selector)?;
        if !descriptor.present {
            findings.push(format!("GDT selector 0x{selector:x} is not present"));
        }
        descriptors.push(descriptor);
    }

    Ok(LinuxDescriptorReport {
        ok: findings.is_empty(),
        findings,
        idt_gates: Vec::new(),
        gdt_descriptors: descriptors,
    })
}

pub fn decode_idt_gate(vector: u8, bytes: [u8; 16]) -> X86IdtGate {
    let offset_low = u64::from(u16::from_le_bytes([bytes[0], bytes[1]]));
    let selector = u16::from_le_bytes([bytes[2], bytes[3]]);
    let type_attr = bytes[5];
    let offset_mid = u64::from(u16::from_le_bytes([bytes[6], bytes[7]]));
    let offset_high = u64::from(u32::from_le_bytes([
        bytes[8], bytes[9], bytes[10], bytes[11],
    ]));
    let offset = offset_low | (offset_mid << 16) | (offset_high << 32);
    X86IdtGate {
        vector,
        offset,
        selector,
        type_attr,
        present: type_attr & 0x80 != 0,
    }
}

pub fn decode_gdt_descriptor(selector: u16, bytes: [u8; 8]) -> X86SegmentDescriptor {
    let limit_low = u32::from(u16::from_le_bytes([bytes[0], bytes[1]]));
    let base_low = u64::from(u16::from_le_bytes([bytes[2], bytes[3]]));
    let base_mid = u64::from(bytes[4]);
    let access = bytes[5];
    let flags_limit = bytes[6];
    let base_high = u64::from(bytes[7]);
    let limit = limit_low | (u32::from(flags_limit & 0x0f) << 16);
    let base = base_low | (base_mid << 16) | (base_high << 24);
    X86SegmentDescriptor {
        selector,
        base,
        limit,
        access,
        flags: flags_limit >> 4,
        present: access & 0x80 != 0,
        executable: access & 0x08 != 0,
        readable_or_writable: access & 0x02 != 0,
        long_mode: flags_limit & 0x20 != 0,
    }
}

fn read_idt_gate(
    memory: &dyn LinuxVirtualMemoryReader,
    address: u64,
    vector: u8,
) -> Result<X86IdtGate, LinuxVmiError> {
    let mut bytes = [0u8; 16];
    memory.read_virtual(address, &mut bytes)?;
    Ok(decode_idt_gate(vector, bytes))
}

fn read_gdt_descriptor(
    memory: &dyn LinuxVirtualMemoryReader,
    address: u64,
    selector: u16,
) -> Result<X86SegmentDescriptor, LinuxVmiError> {
    let mut bytes = [0u8; 8];
    memory.read_virtual(address, &mut bytes)?;
    Ok(decode_gdt_descriptor(selector, bytes))
}
