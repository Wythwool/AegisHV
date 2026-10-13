use std::collections::BTreeSet;

use crate::linux_vmi::{
    address_in_text_ranges, read_u32, read_u64, LinuxTextRange, LinuxVirtualMemoryReader,
    LinuxVmiError,
};
use crate::vmi_linux_profile::{LinuxProfile, LinuxStructFieldKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinuxHookWalkLimits {
    pub max_ftrace_ops: usize,
    pub max_kprobes: usize,
    pub max_kprobe_buckets: usize,
}

impl Default for LinuxHookWalkLimits {
    fn default() -> Self {
        Self {
            max_ftrace_ops: 4096,
            max_kprobes: 8192,
            max_kprobe_buckets: 4096,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxFtraceOp {
    pub address: u64,
    pub callback: u64,
    pub callback_owner: Option<String>,
    pub flags: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxFtraceReport {
    pub ok: bool,
    pub operations: Vec<LinuxFtraceOp>,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxKprobe {
    pub address: u64,
    pub target: u64,
    pub target_owner: Option<String>,
    pub handlers: Vec<LinuxKprobeHandler>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxKprobeHandler {
    pub kind: &'static str,
    pub address: u64,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxKprobeReport {
    pub ok: bool,
    pub probes: Vec<LinuxKprobe>,
    pub findings: Vec<String>,
}

pub fn inspect_linux_ftrace_ops(
    profile: &LinuxProfile,
    memory: &dyn LinuxVirtualMemoryReader,
    slide: u64,
    executable_ranges: &[LinuxTextRange],
    limits: LinuxHookWalkLimits,
) -> Result<LinuxFtraceReport, LinuxVmiError> {
    if executable_ranges.is_empty() {
        return Err(missing_profile("executable text ranges"));
    }
    let head = required_symbol(profile, "ftrace_ops_list", slide, "inspect_ftrace_ops")?;
    let list_offset = required_offset(profile, "ftrace_ops", "list", "inspect_ftrace_ops")?;
    let func_offset = required_offset(profile, "ftrace_ops", "func", "inspect_ftrace_ops")?;
    let flags_offset = optional_offset(profile, "ftrace_ops", "flags");
    let mut findings = Vec::new();
    let mut operations = Vec::new();
    let mut visited = BTreeSet::new();
    let mut node = read_u64(memory, head)?;

    while node != head {
        if operations.len() >= limits.max_ftrace_ops {
            return Err(inconsistent(format!(
                "ftrace_ops list exceeded configured limit {}",
                limits.max_ftrace_ops
            )));
        }
        if !visited.insert(node) {
            return Err(inconsistent(format!(
                "ftrace_ops list looped before returning to head at 0x{node:x}"
            )));
        }
        if node < list_offset {
            return Err(inconsistent(format!(
                "ftrace_ops node 0x{node:x} is smaller than list offset 0x{list_offset:x}"
            )));
        }
        let address = node - list_offset;
        let callback = read_u64(memory, checked_add(address, func_offset)?)?;
        let callback_owner =
            address_in_text_ranges(callback, executable_ranges).map(|range| range.owner.clone());
        if callback != 0 && callback_owner.is_none() {
            findings.push(format!(
                "ftrace callback 0x{callback:x} for ops 0x{address:x} is outside executable kernel/module ranges"
            ));
        }
        let flags = flags_offset
            .map(|offset| read_u32(memory, checked_add(address, offset)?))
            .transpose()?;
        operations.push(LinuxFtraceOp {
            address,
            callback,
            callback_owner,
            flags,
        });
        node = read_u64(memory, node)?;
    }

    Ok(LinuxFtraceReport {
        ok: findings.is_empty(),
        operations,
        findings,
    })
}

pub fn inspect_linux_kprobes(
    profile: &LinuxProfile,
    memory: &dyn LinuxVirtualMemoryReader,
    slide: u64,
    executable_ranges: &[LinuxTextRange],
    buckets: usize,
    limits: LinuxHookWalkLimits,
) -> Result<LinuxKprobeReport, LinuxVmiError> {
    if executable_ranges.is_empty() {
        return Err(missing_profile("executable text ranges"));
    }
    if buckets == 0 || buckets > limits.max_kprobe_buckets {
        return Err(LinuxVmiError::Malformed {
            detail: format!(
                "kprobe bucket count {buckets} must be between 1 and {}",
                limits.max_kprobe_buckets
            ),
        });
    }
    let table = required_symbol(profile, "kprobe_table", slide, "inspect_kprobes")?;
    let hlist_offset = required_offset(profile, "kprobe", "hlist", "inspect_kprobes")?;
    let target_offset = required_offset(profile, "kprobe", "addr", "inspect_kprobes")?;
    let pre_offset = required_offset(profile, "kprobe", "pre_handler", "inspect_kprobes")?;
    let post_offset = optional_offset(profile, "kprobe", "post_handler");
    let fault_offset = optional_offset(profile, "kprobe", "fault_handler");
    let mut findings = Vec::new();
    let mut probes = Vec::new();
    let mut visited = BTreeSet::new();

    for bucket in 0..buckets {
        let head = table
            .checked_add(u64::try_from(bucket).expect("bucket index fits u64") * 8)
            .ok_or_else(|| inconsistent("kprobe table bucket address overflowed"))?;
        let mut node = read_u64(memory, head)?;
        while node != 0 {
            if probes.len() >= limits.max_kprobes {
                return Err(inconsistent(format!(
                    "kprobe table exceeded configured limit {}",
                    limits.max_kprobes
                )));
            }
            if !visited.insert(node) {
                return Err(inconsistent(format!(
                    "kprobe hlist looped at node 0x{node:x}"
                )));
            }
            if node < hlist_offset {
                return Err(inconsistent(format!(
                    "kprobe hlist node 0x{node:x} is smaller than hlist offset 0x{hlist_offset:x}"
                )));
            }
            let address = node - hlist_offset;
            let target = read_u64(memory, checked_add(address, target_offset)?)?;
            let target_owner =
                address_in_text_ranges(target, executable_ranges).map(|range| range.owner.clone());
            if target != 0 && target_owner.is_none() {
                findings.push(format!(
                    "kprobe target 0x{target:x} at 0x{address:x} is outside executable kernel/module ranges"
                ));
            }
            let mut handlers = Vec::new();
            push_handler(
                memory,
                executable_ranges,
                address,
                pre_offset,
                "pre_handler",
                &mut handlers,
                &mut findings,
            )?;
            if let Some(offset) = post_offset {
                push_handler(
                    memory,
                    executable_ranges,
                    address,
                    offset,
                    "post_handler",
                    &mut handlers,
                    &mut findings,
                )?;
            }
            if let Some(offset) = fault_offset {
                push_handler(
                    memory,
                    executable_ranges,
                    address,
                    offset,
                    "fault_handler",
                    &mut handlers,
                    &mut findings,
                )?;
            }
            probes.push(LinuxKprobe {
                address,
                target,
                target_owner,
                handlers,
            });
            node = read_u64(memory, node)?;
        }
    }

    Ok(LinuxKprobeReport {
        ok: findings.is_empty(),
        probes,
        findings,
    })
}

fn push_handler(
    memory: &dyn LinuxVirtualMemoryReader,
    executable_ranges: &[LinuxTextRange],
    probe_address: u64,
    offset: u64,
    kind: &'static str,
    handlers: &mut Vec<LinuxKprobeHandler>,
    findings: &mut Vec<String>,
) -> Result<(), LinuxVmiError> {
    let address = read_u64(memory, checked_add(probe_address, offset)?)?;
    if address == 0 {
        return Ok(());
    }
    let owner = address_in_text_ranges(address, executable_ranges).map(|range| range.owner.clone());
    if owner.is_none() {
        findings.push(format!(
            "kprobe {kind} 0x{address:x} for probe 0x{probe_address:x} is outside executable kernel/module ranges"
        ));
    }
    handlers.push(LinuxKprobeHandler {
        kind,
        address,
        owner,
    });
    Ok(())
}

fn required_symbol(
    profile: &LinuxProfile,
    name: &str,
    slide: u64,
    operation: &'static str,
) -> Result<u64, LinuxVmiError> {
    let symbol = profile
        .symbols()
        .get(name)
        .ok_or_else(|| LinuxVmiError::Unsupported {
            operation,
            detail: format!("profile is missing symbol:{name}"),
        })?;
    symbol
        .virtual_address
        .checked_add(slide)
        .ok_or_else(|| LinuxVmiError::InconsistentSnapshot {
            detail: format!("symbol '{name}' plus KASLR slide overflows u64"),
        })
}

fn required_offset(
    profile: &LinuxProfile,
    struct_name: &str,
    field_name: &str,
    operation: &'static str,
) -> Result<u64, LinuxVmiError> {
    optional_offset(profile, struct_name, field_name).ok_or_else(|| LinuxVmiError::Unsupported {
        operation,
        detail: format!("profile is missing offset {struct_name}.{field_name}"),
    })
}

fn optional_offset(profile: &LinuxProfile, struct_name: &str, field_name: &str) -> Option<u64> {
    let key = LinuxStructFieldKey {
        struct_name: struct_name.to_string(),
        field_name: field_name.to_string(),
    };
    profile.struct_offsets().get(&key).map(|field| field.offset)
}

fn checked_add(base: u64, offset: u64) -> Result<u64, LinuxVmiError> {
    base.checked_add(offset)
        .ok_or_else(|| inconsistent("guest virtual address overflowed"))
}

fn missing_profile(field: &str) -> LinuxVmiError {
    LinuxVmiError::MissingProfileField {
        field: field.to_string(),
    }
}

fn inconsistent(detail: impl Into<String>) -> LinuxVmiError {
    LinuxVmiError::InconsistentSnapshot {
        detail: detail.into(),
    }
}
