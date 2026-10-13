use std::collections::BTreeSet;

use crate::linux_vmi::{
    read_c_string, read_u32, read_u64, LinuxTextRange, LinuxVirtualMemoryReader, LinuxVmiError,
};
use crate::vmi_linux_profile::{LinuxProfile, LinuxStructFieldKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinuxBpfWalkLimits {
    pub max_programs: usize,
}

impl Default for LinuxBpfWalkLimits {
    fn default() -> Self {
        Self { max_programs: 4096 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxBpfProgram {
    pub address: u64,
    pub id: Option<u32>,
    pub name: Option<String>,
    pub program_type: Option<u32>,
    pub jit_start: Option<u64>,
    pub jit_end: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxBpfInventory {
    pub programs: Vec<LinuxBpfProgram>,
    pub jit_ranges: Vec<LinuxTextRange>,
    pub findings: Vec<String>,
}

pub fn inspect_linux_bpf_programs(
    profile: &LinuxProfile,
    memory: &dyn LinuxVirtualMemoryReader,
    slide: u64,
    limits: LinuxBpfWalkLimits,
) -> Result<LinuxBpfInventory, LinuxVmiError> {
    let head = required_symbol(profile, "bpf_prog_list", slide)?;
    let list_offset = required_offset(profile, "bpf_prog", "list")?;
    let aux_offset = optional_offset(profile, "bpf_prog", "aux");
    let type_offset = optional_offset(profile, "bpf_prog", "type");
    let bpf_func_offset = optional_offset(profile, "bpf_prog", "bpf_func");
    let jited_len_offset = optional_offset(profile, "bpf_prog", "jited_len");
    let aux_id_offset = optional_offset(profile, "bpf_prog_aux", "id");
    let aux_name_offset = optional_offset(profile, "bpf_prog_aux", "name");
    let mut programs = Vec::new();
    let mut jit_ranges = Vec::new();
    let mut findings = Vec::new();
    let mut visited = BTreeSet::new();
    let mut node = read_u64(memory, head)?;

    while node != head {
        if programs.len() >= limits.max_programs {
            return Err(inconsistent(format!(
                "BPF program list exceeded configured limit {}",
                limits.max_programs
            )));
        }
        if !visited.insert(node) {
            return Err(inconsistent(format!(
                "BPF program list looped before returning to head at 0x{node:x}"
            )));
        }
        if node < list_offset {
            return Err(inconsistent(format!(
                "BPF program list node 0x{node:x} is smaller than list offset 0x{list_offset:x}"
            )));
        }
        let address = node - list_offset;
        let aux = aux_offset
            .map(|offset| read_u64(memory, checked_add(address, offset)?))
            .transpose()?
            .filter(|value| *value != 0);
        let id = match (aux, aux_id_offset) {
            (Some(aux), Some(offset)) => Some(read_u32(memory, checked_add(aux, offset)?)?),
            _ => None,
        };
        let name = match (aux, aux_name_offset) {
            (Some(aux), Some(offset)) => {
                let value = read_c_string(memory, checked_add(aux, offset)?, 16)?;
                if value.is_empty() {
                    None
                } else {
                    Some(value)
                }
            }
            _ => None,
        };
        let program_type = type_offset
            .map(|offset| read_u32(memory, checked_add(address, offset)?))
            .transpose()?;
        let jit_start = bpf_func_offset
            .map(|offset| read_u64(memory, checked_add(address, offset)?))
            .transpose()?
            .filter(|value| *value != 0);
        let jited_len = jited_len_offset
            .map(|offset| read_u32(memory, checked_add(address, offset)?))
            .transpose()?
            .filter(|value| *value != 0);
        let jit_end = match (jit_start, jited_len) {
            (Some(start), Some(len)) => {
                Some(start.checked_add(u64::from(len)).ok_or_else(|| {
                    LinuxVmiError::InconsistentSnapshot {
                        detail: format!("BPF JIT range for program 0x{address:x} overflows u64"),
                    }
                })?)
            }
            (Some(_), None) => {
                findings.push(format!(
                    "BPF program 0x{address:x} has a JIT entry point but no bounded JIT length"
                ));
                None
            }
            (None, Some(_)) => {
                findings.push(format!(
                    "BPF program 0x{address:x} has a JIT length but no JIT entry point"
                ));
                None
            }
            (None, None) => None,
        };
        if let (Some(start), Some(end)) = (jit_start, jit_end) {
            let owner = match (id, name.as_deref()) {
                (Some(id), Some(name)) => format!("bpf:{id}:{name}"),
                (Some(id), None) => format!("bpf:{id}"),
                (None, Some(name)) => format!("bpf:{name}"),
                (None, None) => format!("bpf:0x{address:x}"),
            };
            jit_ranges.push(LinuxTextRange { owner, start, end });
        }
        programs.push(LinuxBpfProgram {
            address,
            id,
            name,
            program_type,
            jit_start,
            jit_end,
        });
        node = read_u64(memory, node)?;
    }

    Ok(LinuxBpfInventory {
        programs,
        jit_ranges,
        findings,
    })
}

fn required_symbol(profile: &LinuxProfile, name: &str, slide: u64) -> Result<u64, LinuxVmiError> {
    let symbol = profile
        .symbols()
        .get(name)
        .ok_or_else(|| LinuxVmiError::Unsupported {
            operation: "inspect_bpf_programs",
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
) -> Result<u64, LinuxVmiError> {
    optional_offset(profile, struct_name, field_name).ok_or_else(|| LinuxVmiError::Unsupported {
        operation: "inspect_bpf_programs",
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

fn inconsistent(detail: impl Into<String>) -> LinuxVmiError {
    LinuxVmiError::InconsistentSnapshot {
        detail: detail.into(),
    }
}
