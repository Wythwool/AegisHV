use crate::linux_vmi::{
    address_in_text_ranges, read_u64, LinuxTextRange, LinuxVirtualMemoryReader, LinuxVmiError,
};
use crate::vmi::SyscallPathReport;
use crate::vmi_linux_profile::{LinuxProfile, LinuxSymbol};
use crate::vmi_registers::X86_64RegisterSnapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxSyscallEntry {
    pub number: u32,
    pub name: String,
    pub expected_symbol: Option<String>,
    pub handler: u64,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxSyscallTableReport {
    pub ok: bool,
    pub table_address: u64,
    pub entries: Vec<LinuxSyscallEntry>,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxLstarReport {
    pub ok: bool,
    pub lstar: u64,
    pub expected_symbol: String,
    pub findings: Vec<String>,
}

pub fn inspect_linux_syscall_table(
    profile: &LinuxProfile,
    memory: &dyn LinuxVirtualMemoryReader,
    slide: u64,
    executable_ranges: &[LinuxTextRange],
) -> Result<LinuxSyscallTableReport, LinuxVmiError> {
    if profile.syscalls_by_number().is_empty() {
        return Err(LinuxVmiError::MissingProfileField {
            field: "syscall entries".to_string(),
        });
    }
    if executable_ranges.is_empty() {
        return Err(LinuxVmiError::MissingProfileField {
            field: "executable text ranges".to_string(),
        });
    }
    let table_address = slid_symbol(profile, "sys_call_table", slide)?;
    let mut entries = Vec::new();
    let mut findings = Vec::new();

    for syscall in profile.syscalls_by_number().values() {
        let entry_address = table_address
            .checked_add(u64::from(syscall.number) * 8)
            .ok_or_else(|| LinuxVmiError::InconsistentSnapshot {
                detail: format!("sys_call_table entry {} address overflowed", syscall.number),
            })?;
        let handler = read_u64(memory, entry_address)?;
        let owner = address_in_text_ranges(handler, executable_ranges).map(|range| {
            if range.owner == "vmlinux" {
                "vmlinux".to_string()
            } else {
                format!("module:{}", range.owner)
            }
        });
        if owner.is_none() {
            findings.push(format!(
                "syscall {} ({}) handler 0x{handler:x} is outside executable kernel/module ranges",
                syscall.number, syscall.name
            ));
        }
        entries.push(LinuxSyscallEntry {
            number: syscall.number,
            name: syscall.name.clone(),
            expected_symbol: syscall.symbol_name.clone(),
            handler,
            owner,
        });
    }

    Ok(LinuxSyscallTableReport {
        ok: findings.is_empty(),
        table_address,
        entries,
        findings,
    })
}

pub fn inspect_linux_lstar(
    profile: &LinuxProfile,
    regs: &X86_64RegisterSnapshot,
    slide: u64,
    executable_ranges: &[LinuxTextRange],
) -> Result<LinuxLstarReport, LinuxVmiError> {
    let lstar = regs.lstar()?;
    let expected = profile.symbols().get("entry_SYSCALL_64").ok_or_else(|| {
        LinuxVmiError::MissingProfileField {
            field: "symbol:entry_SYSCALL_64".to_string(),
        }
    })?;
    let expected_start = expected.virtual_address.checked_add(slide).ok_or_else(|| {
        LinuxVmiError::InconsistentSnapshot {
            detail: "entry_SYSCALL_64 plus KASLR slide overflows u64".to_string(),
        }
    })?;
    let expected_end = expected_range_end(expected, expected_start)?;
    let mut findings = Vec::new();

    if !(expected_start <= lstar && lstar < expected_end) {
        findings.push(format!(
            "MSR_LSTAR target 0x{lstar:x} is outside entry_SYSCALL_64 range 0x{expected_start:x}..0x{expected_end:x}"
        ));
    }
    if address_in_text_ranges(lstar, executable_ranges).is_none() {
        findings.push(format!(
            "MSR_LSTAR target 0x{lstar:x} is outside executable kernel/module ranges"
        ));
    }

    Ok(LinuxLstarReport {
        ok: findings.is_empty(),
        lstar,
        expected_symbol: "entry_SYSCALL_64".to_string(),
        findings,
    })
}

pub fn linux_syscall_path_report(
    profile: &LinuxProfile,
    table: &LinuxSyscallTableReport,
    lstar: &LinuxLstarReport,
) -> SyscallPathReport {
    let mut findings = Vec::new();
    findings.extend(lstar.findings.clone());
    findings.extend(table.findings.clone());
    SyscallPathReport {
        ok: findings.is_empty(),
        os: "linux".to_string(),
        entry: Some(lstar.lstar),
        table: Some(table.table_address),
        findings: if findings.is_empty() {
            let release = &profile.linux_identity().kernel_release;
            vec![format!("linux syscall path matched profile {release}")]
        } else {
            findings
        },
    }
}

fn expected_range_end(symbol: &LinuxSymbol, start: u64) -> Result<u64, LinuxVmiError> {
    let size = symbol.size.unwrap_or(1);
    start
        .checked_add(size)
        .ok_or_else(|| LinuxVmiError::InconsistentSnapshot {
            detail: format!("symbol '{}' range overflows u64", symbol.name),
        })
}

fn slid_symbol(profile: &LinuxProfile, name: &str, slide: u64) -> Result<u64, LinuxVmiError> {
    let symbol = profile
        .symbols()
        .get(name)
        .ok_or_else(|| LinuxVmiError::MissingProfileField {
            field: format!("symbol:{name}"),
        })?;
    symbol
        .virtual_address
        .checked_add(slide)
        .ok_or_else(|| LinuxVmiError::InconsistentSnapshot {
            detail: format!("symbol '{name}' plus slide overflows u64"),
        })
}
