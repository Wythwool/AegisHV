use crate::vmi::SyscallPathReport;
use crate::vmi_registers::X86_64RegisterSnapshot;
use crate::windows_profile::{WindowsProfile, WindowsSymbol};
use crate::windows_vmi::{
    address_in_windows_text_ranges, read_u32, read_u64, symbol_address, WindowsTextRange,
    WindowsVirtualMemoryReader, WindowsVmiError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsSsdtEntry {
    pub number: u32,
    pub name: String,
    pub expected_symbol: Option<String>,
    pub raw_offset: i32,
    pub handler: u64,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsSsdtReport {
    pub ok: bool,
    pub descriptor_address: u64,
    pub table_address: u64,
    pub service_count: u32,
    pub entries: Vec<WindowsSsdtEntry>,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsLstarReport {
    pub ok: bool,
    pub lstar: u64,
    pub expected_symbol: String,
    pub findings: Vec<String>,
}

pub fn inspect_windows_ssdt(
    profile: &WindowsProfile,
    memory: &dyn WindowsVirtualMemoryReader,
    nt_base: u64,
    executable_ranges: &[WindowsTextRange],
) -> Result<WindowsSsdtReport, WindowsVmiError> {
    if profile.syscalls_by_number().is_empty() {
        return Err(WindowsVmiError::MissingProfileField {
            field: "syscall entries".to_string(),
        });
    }
    if executable_ranges.is_empty() {
        return Err(WindowsVmiError::MissingProfileField {
            field: "executable text ranges".to_string(),
        });
    }

    let descriptor_address = symbol_address(profile, "KeServiceDescriptorTable", nt_base)?;
    let table_address = read_u64(memory, descriptor_address)?;
    let service_count = read_u32(memory, checked_add(descriptor_address, 0x10)?)?;
    if table_address == 0 || service_count == 0 {
        return Err(WindowsVmiError::InconsistentSnapshot {
            detail: "KeServiceDescriptorTable has an empty service table".to_string(),
        });
    }

    let mut entries = Vec::new();
    let mut findings = Vec::new();
    for syscall in profile.syscalls_by_number().values() {
        if syscall.number >= service_count {
            findings.push(format!(
                "syscall {} ({}) is outside SSDT service count {service_count}",
                syscall.number, syscall.name
            ));
            continue;
        }

        let entry_address = checked_add(table_address, u64::from(syscall.number) * 4)?;
        let raw_offset = read_i32(memory, entry_address)?;
        let handler = decode_ssdt_handler(table_address, raw_offset)?;
        let owner = address_in_windows_text_ranges(handler, executable_ranges)
            .map(|range| range.owner.clone());
        if owner.is_none() {
            findings.push(format!(
                "syscall {} ({}) handler 0x{handler:x} is outside executable Windows ranges",
                syscall.number, syscall.name
            ));
        }

        if let Some(symbol_name) = syscall.symbol_name.as_deref() {
            if let Some(symbol) = profile.symbols().get(symbol_name) {
                let start = symbol_address(profile, symbol_name, nt_base)?;
                let end = expected_range_end(symbol, start)?;
                if !(start <= handler && handler < end) {
                    findings.push(format!(
                        "syscall {} ({}) handler 0x{handler:x} is outside expected symbol {symbol_name} range 0x{start:x}..0x{end:x}",
                        syscall.number, syscall.name
                    ));
                }
            }
        }

        entries.push(WindowsSsdtEntry {
            number: syscall.number,
            name: syscall.name.clone(),
            expected_symbol: syscall.symbol_name.clone(),
            raw_offset,
            handler,
            owner,
        });
    }

    Ok(WindowsSsdtReport {
        ok: findings.is_empty(),
        descriptor_address,
        table_address,
        service_count,
        entries,
        findings,
    })
}

pub fn inspect_windows_lstar(
    profile: &WindowsProfile,
    regs: &X86_64RegisterSnapshot,
    nt_base: u64,
    executable_ranges: &[WindowsTextRange],
) -> Result<WindowsLstarReport, WindowsVmiError> {
    let lstar = regs.lstar()?;
    let expected = profile.symbols().get("KiSystemCall64").ok_or_else(|| {
        WindowsVmiError::MissingProfileField {
            field: "symbol:KiSystemCall64".to_string(),
        }
    })?;
    let expected_start = symbol_address(profile, "KiSystemCall64", nt_base)?;
    let expected_end = expected_range_end(expected, expected_start)?;
    let mut findings = Vec::new();

    if !(expected_start <= lstar && lstar < expected_end) {
        findings.push(format!(
            "MSR_LSTAR target 0x{lstar:x} is outside KiSystemCall64 range 0x{expected_start:x}..0x{expected_end:x}"
        ));
    }
    if address_in_windows_text_ranges(lstar, executable_ranges).is_none() {
        findings.push(format!(
            "MSR_LSTAR target 0x{lstar:x} is outside executable Windows ranges"
        ));
    }

    Ok(WindowsLstarReport {
        ok: findings.is_empty(),
        lstar,
        expected_symbol: "KiSystemCall64".to_string(),
        findings,
    })
}

pub fn windows_syscall_path_report(
    profile: &WindowsProfile,
    ssdt: &WindowsSsdtReport,
    lstar: &WindowsLstarReport,
) -> SyscallPathReport {
    let mut findings = Vec::new();
    findings.extend(lstar.findings.clone());
    findings.extend(ssdt.findings.clone());
    SyscallPathReport {
        ok: findings.is_empty(),
        os: "windows".to_string(),
        entry: Some(lstar.lstar),
        table: Some(ssdt.table_address),
        findings: if findings.is_empty() {
            let build = &profile.windows_identity().build;
            vec![format!("windows syscall path matched profile {build}")]
        } else {
            findings
        },
    }
}

fn read_i32(memory: &dyn WindowsVirtualMemoryReader, address: u64) -> Result<i32, WindowsVmiError> {
    let value = read_u32(memory, address)?;
    Ok(i32::from_le_bytes(value.to_le_bytes()))
}

fn decode_ssdt_handler(table_address: u64, raw_offset: i32) -> Result<u64, WindowsVmiError> {
    let relative = raw_offset >> 4;
    if relative >= 0 {
        table_address.checked_add(relative as u64).ok_or_else(|| {
            WindowsVmiError::InconsistentSnapshot {
                detail: "SSDT handler address overflowed".to_string(),
            }
        })
    } else {
        table_address
            .checked_sub(i64::from(relative).unsigned_abs())
            .ok_or_else(|| WindowsVmiError::InconsistentSnapshot {
                detail: "SSDT handler address underflowed".to_string(),
            })
    }
}

fn expected_range_end(symbol: &WindowsSymbol, start: u64) -> Result<u64, WindowsVmiError> {
    let size = symbol.size.unwrap_or(1);
    start
        .checked_add(size)
        .ok_or_else(|| WindowsVmiError::InconsistentSnapshot {
            detail: format!("symbol '{}' range overflows u64", symbol.name),
        })
}

fn checked_add(base: u64, offset: u64) -> Result<u64, WindowsVmiError> {
    base.checked_add(offset)
        .ok_or_else(|| WindowsVmiError::InconsistentSnapshot {
            detail: "guest virtual address overflowed".to_string(),
        })
}
