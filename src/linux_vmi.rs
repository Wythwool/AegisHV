use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::vmi::{MemoryReadError, ProfileError, RegisterReadError, VcpuId, VmiErrorKind};
use crate::vmi_linux_profile::{LinuxProfile, LinuxStructFieldKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxVmiError {
    Unsupported {
        operation: &'static str,
        detail: String,
    },
    MissingProfileField {
        field: String,
    },
    Malformed {
        detail: String,
    },
    InconsistentSnapshot {
        detail: String,
    },
    Memory(MemoryReadError),
    Profile(ProfileError),
    Registers(RegisterReadError),
    Backend {
        detail: String,
    },
}

impl LinuxVmiError {
    pub fn kind(&self) -> VmiErrorKind {
        match self {
            Self::Unsupported { .. } => VmiErrorKind::Unsupported,
            Self::MissingProfileField { .. } => VmiErrorKind::MissingProfile,
            Self::Malformed { .. } => VmiErrorKind::Malformed,
            Self::InconsistentSnapshot { .. } => VmiErrorKind::InconsistentSnapshot,
            Self::Memory(err) => err.kind(),
            Self::Profile(err) => err.kind(),
            Self::Registers(err) => err.kind(),
            Self::Backend { .. } => VmiErrorKind::Backend,
        }
    }
}

impl fmt::Display for LinuxVmiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { operation, detail } => {
                write!(
                    f,
                    "Linux VMI operation '{operation}' is unsupported: {detail}"
                )
            }
            Self::MissingProfileField { field } => {
                write!(f, "Linux VMI profile is missing required field '{field}'")
            }
            Self::Malformed { detail } => write!(f, "Linux VMI input is malformed: {detail}"),
            Self::InconsistentSnapshot { detail } => {
                write!(f, "Linux VMI snapshot is inconsistent: {detail}")
            }
            Self::Memory(err) => write!(f, "Linux VMI memory read failed: {err}"),
            Self::Profile(err) => write!(f, "Linux VMI profile error: {err}"),
            Self::Registers(err) => write!(f, "Linux VMI register error: {err}"),
            Self::Backend { detail } => write!(f, "Linux VMI backend error: {detail}"),
        }
    }
}

impl Error for LinuxVmiError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(err) => Some(err),
            Self::Profile(err) => Some(err),
            Self::Registers(err) => Some(err),
            Self::Unsupported { .. }
            | Self::MissingProfileField { .. }
            | Self::Malformed { .. }
            | Self::InconsistentSnapshot { .. }
            | Self::Backend { .. } => None,
        }
    }
}

impl From<MemoryReadError> for LinuxVmiError {
    fn from(value: MemoryReadError) -> Self {
        Self::Memory(value)
    }
}

impl From<ProfileError> for LinuxVmiError {
    fn from(value: ProfileError) -> Self {
        Self::Profile(value)
    }
}

impl From<RegisterReadError> for LinuxVmiError {
    fn from(value: RegisterReadError) -> Self {
        Self::Registers(value)
    }
}

pub trait LinuxVirtualMemoryReader {
    fn read_virtual(&self, address: u64, buf: &mut [u8]) -> Result<(), LinuxVmiError>;
}

#[derive(Debug, Clone, Default)]
pub struct SyntheticLinuxVirtualMemory {
    ranges: Vec<SyntheticVirtualRange>,
}

#[derive(Debug, Clone)]
struct SyntheticVirtualRange {
    start: u64,
    end: u64,
    bytes: Vec<u8>,
}

impl SyntheticLinuxVirtualMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn map_range(
        &mut self,
        address: u64,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<(), LinuxVmiError> {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(LinuxVmiError::Malformed {
                detail: "synthetic virtual memory range must not be empty".to_string(),
            });
        }
        let len = u64::try_from(bytes.len()).map_err(|_| LinuxVmiError::Malformed {
            detail: "synthetic virtual memory range is too large".to_string(),
        })?;
        let end = address.checked_add(len).ok_or(LinuxVmiError::Malformed {
            detail: "synthetic virtual memory range overflows u64".to_string(),
        })?;
        if self
            .ranges
            .iter()
            .any(|range| address < range.end && range.start < end)
        {
            return Err(LinuxVmiError::InconsistentSnapshot {
                detail: format!("overlapping synthetic virtual memory range at 0x{address:x}"),
            });
        }
        self.ranges.push(SyntheticVirtualRange {
            start: address,
            end,
            bytes,
        });
        self.ranges.sort_by_key(|range| range.start);
        Ok(())
    }

    fn range_containing(&self, address: u64) -> Option<&SyntheticVirtualRange> {
        self.ranges
            .iter()
            .find(|range| range.start <= address && address < range.end)
    }
}

impl LinuxVirtualMemoryReader for SyntheticLinuxVirtualMemory {
    fn read_virtual(&self, address: u64, buf: &mut [u8]) -> Result<(), LinuxVmiError> {
        if buf.is_empty() {
            return Err(LinuxVmiError::Malformed {
                detail: "virtual memory read length must not be zero".to_string(),
            });
        }
        let end = address
            .checked_add(
                u64::try_from(buf.len()).map_err(|_| LinuxVmiError::Malformed {
                    detail: "virtual memory read length is too large".to_string(),
                })?,
            )
            .ok_or(LinuxVmiError::Malformed {
                detail: "virtual memory read range overflows u64".to_string(),
            })?;
        let mut cursor = address;
        let mut copied = 0usize;
        while cursor < end {
            let Some(range) = self.range_containing(cursor) else {
                return Err(LinuxVmiError::Memory(MemoryReadError::Unmapped {
                    gpa: crate::vmi::GuestPhysical(cursor),
                    len: buf.len().saturating_sub(copied),
                }));
            };
            let range_offset =
                usize::try_from(cursor - range.start).expect("range offset fits usize");
            let readable = range
                .bytes
                .len()
                .saturating_sub(range_offset)
                .min(buf.len() - copied);
            buf[copied..copied + readable]
                .copy_from_slice(&range.bytes[range_offset..range_offset + readable]);
            cursor = cursor
                .checked_add(u64::try_from(readable).expect("read length fits u64"))
                .ok_or(LinuxVmiError::Malformed {
                    detail: "virtual memory read cursor overflowed".to_string(),
                })?;
            copied += readable;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxTask {
    pub address: u64,
    pub pid: i32,
    pub tgid: i32,
    pub comm: String,
    pub mm: Option<u64>,
    pub cred: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxModule {
    pub address: u64,
    pub name: String,
    pub state: i32,
    pub text_base: u64,
    pub text_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxTextRange {
    pub owner: String,
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinuxWalkLimits {
    pub max_tasks: usize,
    pub max_modules: usize,
}

impl Default for LinuxWalkLimits {
    fn default() -> Self {
        Self {
            max_tasks: 4096,
            max_modules: 1024,
        }
    }
}

pub fn walk_linux_tasks(
    profile: &LinuxProfile,
    memory: &dyn LinuxVirtualMemoryReader,
    slide: u64,
    limits: LinuxWalkLimits,
) -> Result<Vec<LinuxTask>, LinuxVmiError> {
    let init_task = slid_symbol(profile, "init_task", slide)?;
    let tasks_offset = field_offset(profile, "task_struct", "tasks")?;
    let head = init_task
        .checked_add(tasks_offset)
        .ok_or_else(|| inconsistent("init_task tasks list address overflowed"))?;
    walk_task_list(profile, memory, head, tasks_offset, limits.max_tasks)
}

pub fn resolve_linux_current_task(
    profile: &LinuxProfile,
    memory: &dyn LinuxVirtualMemoryReader,
    slide: u64,
    vcpu: VcpuId,
) -> Result<LinuxTask, LinuxVmiError> {
    let symbol_name = format!("aegishv_current_task_vcpu{}", vcpu.0);
    let pointer_address = match profile.symbols().get(&symbol_name) {
        Some(_) => slid_symbol(profile, &symbol_name, slide)?,
        None => slid_symbol(profile, "aegishv_current_task", slide).map_err(|_| {
            LinuxVmiError::Unsupported {
                operation: "resolve_current_task",
                detail: format!(
                    "profile has no current-task pointer symbol for vCPU {}",
                    vcpu.0
                ),
            }
        })?,
    };
    let task_address = read_u64(memory, pointer_address)?;
    if task_address == 0 {
        return Err(LinuxVmiError::InconsistentSnapshot {
            detail: format!("current task pointer for vCPU {} is null", vcpu.0),
        });
    }
    read_task(profile, memory, task_address)
}

pub fn walk_linux_modules(
    profile: &LinuxProfile,
    memory: &dyn LinuxVirtualMemoryReader,
    slide: u64,
    limits: LinuxWalkLimits,
) -> Result<Vec<LinuxModule>, LinuxVmiError> {
    let head = slid_symbol(profile, "modules", slide)?;
    let list_offset = field_offset(profile, "module", "list")?;
    let mut out = Vec::new();
    let mut visited = BTreeSet::new();
    let mut node = read_u64(memory, head)?;

    while node != head {
        if out.len() >= limits.max_modules {
            return Err(inconsistent(format!(
                "module list exceeded configured limit {}",
                limits.max_modules
            )));
        }
        if !visited.insert(node) {
            return Err(inconsistent(format!(
                "module list looped before returning to head at 0x{node:x}"
            )));
        }
        if node < list_offset {
            return Err(inconsistent(format!(
                "module list node 0x{node:x} is smaller than list offset 0x{list_offset:x}"
            )));
        }
        let module_address = node - list_offset;
        out.push(read_module(profile, memory, module_address)?);
        node = read_u64(memory, node)?;
    }

    Ok(out)
}

pub fn linux_executable_ranges(
    profile: &LinuxProfile,
    modules: &[LinuxModule],
    slide: u64,
) -> Result<Vec<LinuxTextRange>, LinuxVmiError> {
    let mut ranges = Vec::new();
    if let Some(stext) = profile.symbols().get("_stext") {
        let end = profile
            .symbols()
            .get("_etext")
            .map(|symbol| symbol.virtual_address)
            .or_else(|| stext.size.map(|size| stext.virtual_address + size));
        if let Some(end) = end {
            ranges.push(LinuxTextRange {
                owner: "vmlinux".to_string(),
                start: stext.virtual_address + slide,
                end: end + slide,
            });
        }
    }
    for module in modules {
        ranges.push(LinuxTextRange {
            owner: module.name.clone(),
            start: module.text_base,
            end: module.text_base.saturating_add(module.text_size),
        });
    }
    if ranges.is_empty() {
        return Err(LinuxVmiError::MissingProfileField {
            field: "vmlinux text range or module text ranges".to_string(),
        });
    }
    Ok(ranges)
}

pub fn address_in_text_ranges(address: u64, ranges: &[LinuxTextRange]) -> Option<&LinuxTextRange> {
    ranges
        .iter()
        .find(|range| range.start <= address && address < range.end)
}

fn walk_task_list(
    profile: &LinuxProfile,
    memory: &dyn LinuxVirtualMemoryReader,
    head: u64,
    tasks_offset: u64,
    max_tasks: usize,
) -> Result<Vec<LinuxTask>, LinuxVmiError> {
    let init_task = head
        .checked_sub(tasks_offset)
        .ok_or_else(|| inconsistent("init_task head is smaller than tasks offset"))?;
    let mut out = vec![read_task(profile, memory, init_task)?];
    let mut visited = BTreeSet::from([head]);
    let mut node = read_u64(memory, head)?;

    while node != head {
        if out.len() >= max_tasks {
            return Err(inconsistent(format!(
                "task list exceeded configured limit {max_tasks}"
            )));
        }
        if !visited.insert(node) {
            return Err(inconsistent(format!(
                "task list looped before returning to init_task at 0x{node:x}"
            )));
        }
        if node < tasks_offset {
            return Err(inconsistent(format!(
                "task list node 0x{node:x} is smaller than tasks offset 0x{tasks_offset:x}"
            )));
        }
        let task_address = node - tasks_offset;
        out.push(read_task(profile, memory, task_address)?);
        node = read_u64(memory, node)?;
    }

    Ok(out)
}

fn read_task(
    profile: &LinuxProfile,
    memory: &dyn LinuxVirtualMemoryReader,
    address: u64,
) -> Result<LinuxTask, LinuxVmiError> {
    let pid = read_i32(
        memory,
        checked_add(address, field_offset(profile, "task_struct", "pid")?)?,
    )?;
    let tgid_offset = optional_field_offset(profile, "task_struct", "tgid")
        .unwrap_or(field_offset(profile, "task_struct", "pid")?);
    let tgid = read_i32(memory, checked_add(address, tgid_offset)?)?;
    let comm_field = field(profile, "task_struct", "comm")?;
    let comm = read_c_string(
        memory,
        checked_add(address, comm_field.offset)?,
        field_size(comm_field, 16)?,
    )?;
    let mm = optional_pointer(profile, memory, address, "task_struct", "mm")?;
    let cred = optional_pointer(profile, memory, address, "task_struct", "cred")?;
    Ok(LinuxTask {
        address,
        pid,
        tgid,
        comm,
        mm,
        cred,
    })
}

fn read_module(
    profile: &LinuxProfile,
    memory: &dyn LinuxVirtualMemoryReader,
    address: u64,
) -> Result<LinuxModule, LinuxVmiError> {
    let name_field = field(profile, "module", "name")?;
    let name = read_c_string(
        memory,
        checked_add(address, name_field.offset)?,
        field_size(name_field, 56)?,
    )?;
    let state = read_i32(
        memory,
        checked_add(address, field_offset(profile, "module", "state")?)?,
    )?;
    let text_base = if let Some(offset) = optional_field_offset(profile, "module", "text_base") {
        read_u64(memory, checked_add(address, offset)?)?
    } else {
        read_u64(
            memory,
            checked_add(address, field_offset(profile, "module", "core_base")?)?,
        )?
    };
    let text_size = read_u64(
        memory,
        checked_add(address, field_offset(profile, "module", "text_size")?)?,
    )?;
    if text_size == 0 {
        return Err(inconsistent(format!(
            "module '{name}' has an empty executable text range"
        )));
    }
    Ok(LinuxModule {
        address,
        name,
        state,
        text_base,
        text_size,
    })
}

fn optional_pointer(
    profile: &LinuxProfile,
    memory: &dyn LinuxVirtualMemoryReader,
    base: u64,
    struct_name: &str,
    field_name: &str,
) -> Result<Option<u64>, LinuxVmiError> {
    optional_field_offset(profile, struct_name, field_name)
        .map(|offset| read_u64(memory, checked_add(base, offset)?))
        .transpose()
}

fn field<'a>(
    profile: &'a LinuxProfile,
    struct_name: &str,
    field_name: &str,
) -> Result<&'a crate::vmi_linux_profile::LinuxStructOffset, LinuxVmiError> {
    let key = LinuxStructFieldKey {
        struct_name: struct_name.to_string(),
        field_name: field_name.to_string(),
    };
    profile
        .struct_offsets()
        .get(&key)
        .ok_or_else(|| LinuxVmiError::MissingProfileField {
            field: format!("{struct_name}.{field_name}"),
        })
}

fn field_offset(
    profile: &LinuxProfile,
    struct_name: &str,
    field_name: &str,
) -> Result<u64, LinuxVmiError> {
    Ok(field(profile, struct_name, field_name)?.offset)
}

fn optional_field_offset(
    profile: &LinuxProfile,
    struct_name: &str,
    field_name: &str,
) -> Option<u64> {
    let key = LinuxStructFieldKey {
        struct_name: struct_name.to_string(),
        field_name: field_name.to_string(),
    };
    profile.struct_offsets().get(&key).map(|field| field.offset)
}

fn field_size(
    field: &crate::vmi_linux_profile::LinuxStructOffset,
    fallback: usize,
) -> Result<usize, LinuxVmiError> {
    field
        .size
        .map(|size| {
            usize::try_from(size).map_err(|_| LinuxVmiError::Malformed {
                detail: format!(
                    "field '{}.{}' size does not fit this target",
                    field.struct_name, field.field_name
                ),
            })
        })
        .unwrap_or(Ok(fallback))
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
        .ok_or_else(|| inconsistent(format!("symbol '{name}' plus slide overflows u64")))
}

pub fn read_u64(memory: &dyn LinuxVirtualMemoryReader, address: u64) -> Result<u64, LinuxVmiError> {
    let mut bytes = [0u8; 8];
    memory.read_virtual(address, &mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

pub fn read_u32(memory: &dyn LinuxVirtualMemoryReader, address: u64) -> Result<u32, LinuxVmiError> {
    let mut bytes = [0u8; 4];
    memory.read_virtual(address, &mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

pub fn read_i32(memory: &dyn LinuxVirtualMemoryReader, address: u64) -> Result<i32, LinuxVmiError> {
    let mut bytes = [0u8; 4];
    memory.read_virtual(address, &mut bytes)?;
    Ok(i32::from_le_bytes(bytes))
}

pub fn read_c_string(
    memory: &dyn LinuxVirtualMemoryReader,
    address: u64,
    max_len: usize,
) -> Result<String, LinuxVmiError> {
    if max_len == 0 || max_len > 4096 {
        return Err(LinuxVmiError::Malformed {
            detail: format!("invalid bounded C string length {max_len}"),
        });
    }
    let mut bytes = vec![0u8; max_len];
    memory.read_virtual(address, &mut bytes)?;
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8(bytes[..end].to_vec()).map_err(|_| LinuxVmiError::Malformed {
        detail: format!("guest string at 0x{address:x} is not UTF-8"),
    })
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
