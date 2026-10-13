use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::vmi::{MemoryReadError, RegisterReadError, VmiErrorKind};
use crate::vmi_registers::X86_64RegisterSnapshot;
use crate::windows_profile::{WindowsProfile, WindowsStructFieldKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowsVmiError {
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
    Registers(RegisterReadError),
    Backend {
        detail: String,
    },
}

impl WindowsVmiError {
    pub fn kind(&self) -> VmiErrorKind {
        match self {
            Self::Unsupported { .. } => VmiErrorKind::Unsupported,
            Self::MissingProfileField { .. } => VmiErrorKind::MissingProfile,
            Self::Malformed { .. } => VmiErrorKind::Malformed,
            Self::InconsistentSnapshot { .. } => VmiErrorKind::InconsistentSnapshot,
            Self::Memory(err) => err.kind(),
            Self::Registers(err) => err.kind(),
            Self::Backend { .. } => VmiErrorKind::Backend,
        }
    }
}

impl fmt::Display for WindowsVmiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { operation, detail } => {
                write!(
                    f,
                    "Windows VMI operation '{operation}' is unsupported: {detail}"
                )
            }
            Self::MissingProfileField { field } => {
                write!(f, "Windows VMI profile is missing required field '{field}'")
            }
            Self::Malformed { detail } => write!(f, "Windows VMI input is malformed: {detail}"),
            Self::InconsistentSnapshot { detail } => {
                write!(f, "Windows VMI snapshot is inconsistent: {detail}")
            }
            Self::Memory(err) => write!(f, "Windows VMI memory read failed: {err}"),
            Self::Registers(err) => write!(f, "Windows VMI register read failed: {err}"),
            Self::Backend { detail } => write!(f, "Windows VMI backend error: {detail}"),
        }
    }
}

impl Error for WindowsVmiError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(err) => Some(err),
            Self::Registers(err) => Some(err),
            Self::Unsupported { .. }
            | Self::MissingProfileField { .. }
            | Self::Malformed { .. }
            | Self::InconsistentSnapshot { .. }
            | Self::Backend { .. } => None,
        }
    }
}

impl From<MemoryReadError> for WindowsVmiError {
    fn from(value: MemoryReadError) -> Self {
        Self::Memory(value)
    }
}

impl From<RegisterReadError> for WindowsVmiError {
    fn from(value: RegisterReadError) -> Self {
        Self::Registers(value)
    }
}

pub trait WindowsVirtualMemoryReader {
    fn read_virtual(&self, address: u64, buf: &mut [u8]) -> Result<(), WindowsVmiError>;
}

#[derive(Debug, Clone, Default)]
pub struct SyntheticWindowsVirtualMemory {
    ranges: Vec<SyntheticVirtualRange>,
}

#[derive(Debug, Clone)]
struct SyntheticVirtualRange {
    start: u64,
    end: u64,
    bytes: Vec<u8>,
}

impl SyntheticWindowsVirtualMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn map_range(
        &mut self,
        address: u64,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<(), WindowsVmiError> {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(WindowsVmiError::Malformed {
                detail: "synthetic virtual memory range must not be empty".to_string(),
            });
        }
        let len = u64::try_from(bytes.len()).map_err(|_| WindowsVmiError::Malformed {
            detail: "synthetic virtual memory range is too large".to_string(),
        })?;
        let end = address
            .checked_add(len)
            .ok_or_else(|| WindowsVmiError::Malformed {
                detail: "synthetic virtual memory range overflows u64".to_string(),
            })?;
        if self
            .ranges
            .iter()
            .any(|range| address < range.end && range.start < end)
        {
            return Err(WindowsVmiError::InconsistentSnapshot {
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

impl WindowsVirtualMemoryReader for SyntheticWindowsVirtualMemory {
    fn read_virtual(&self, address: u64, buf: &mut [u8]) -> Result<(), WindowsVmiError> {
        if buf.is_empty() {
            return Err(WindowsVmiError::Malformed {
                detail: "virtual memory read length must not be zero".to_string(),
            });
        }
        let end = address
            .checked_add(
                u64::try_from(buf.len()).map_err(|_| WindowsVmiError::Malformed {
                    detail: "virtual memory read length is too large".to_string(),
                })?,
            )
            .ok_or_else(|| WindowsVmiError::Malformed {
                detail: "virtual memory read range overflows u64".to_string(),
            })?;
        let mut cursor = address;
        let mut copied = 0usize;
        while cursor < end {
            let Some(range) = self.range_containing(cursor) else {
                return Err(WindowsVmiError::Memory(MemoryReadError::Unmapped {
                    gpa: crate::vmi::GuestPhysical(cursor),
                    len: buf.len().saturating_sub(copied),
                }));
            };
            let offset = usize::try_from(cursor - range.start).expect("range offset fits usize");
            let readable = range
                .bytes
                .len()
                .saturating_sub(offset)
                .min(buf.len() - copied);
            buf[copied..copied + readable].copy_from_slice(&range.bytes[offset..offset + readable]);
            cursor = cursor
                .checked_add(u64::try_from(readable).expect("read length fits u64"))
                .ok_or_else(|| WindowsVmiError::Malformed {
                    detail: "virtual memory read cursor overflowed".to_string(),
                })?;
            copied += readable;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsProcess {
    pub address: u64,
    pub pid: u64,
    pub image_name: String,
    pub directory_table_base: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsModule {
    pub address: u64,
    pub name: String,
    pub dll_base: u64,
    pub size_of_image: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsTextRange {
    pub owner: String,
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsWalkLimits {
    pub max_processes: usize,
    pub max_modules: usize,
}

impl Default for WindowsWalkLimits {
    fn default() -> Self {
        Self {
            max_processes: 4096,
            max_modules: 2048,
        }
    }
}

pub fn resolve_windows_ntoskrnl_base(
    memory: &dyn WindowsVirtualMemoryReader,
    candidates: &[u64],
) -> Result<u64, WindowsVmiError> {
    let mut matched = Vec::new();
    for &candidate in candidates {
        if candidate != 0 && has_pe_header(memory, candidate)? {
            matched.push(candidate);
        }
    }
    match matched.as_slice() {
        [base] => Ok(*base),
        [] => Err(WindowsVmiError::InconsistentSnapshot {
            detail: "no ntoskrnl PE header matched the supplied base candidates".to_string(),
        }),
        _ => Err(WindowsVmiError::InconsistentSnapshot {
            detail: "more than one ntoskrnl PE header matched the supplied base candidates"
                .to_string(),
        }),
    }
}

pub fn walk_windows_processes(
    profile: &WindowsProfile,
    memory: &dyn WindowsVirtualMemoryReader,
    nt_base: u64,
    limits: WindowsWalkLimits,
) -> Result<Vec<WindowsProcess>, WindowsVmiError> {
    let pointer = symbol_address(profile, "PsInitialSystemProcess", nt_base)?;
    let system_process = read_u64(memory, pointer)?;
    if system_process == 0 {
        return Err(inconsistent("PsInitialSystemProcess points to null"));
    }
    let list_offset = field_offset(profile, "EPROCESS", "ActiveProcessLinks")?;
    let head = checked_add(system_process, list_offset)?;
    walk_process_list(profile, memory, head, list_offset, limits.max_processes)
}

pub fn resolve_windows_current_process(
    profile: &WindowsProfile,
    memory: &dyn WindowsVirtualMemoryReader,
    nt_base: u64,
    regs: &X86_64RegisterSnapshot,
    limits: WindowsWalkLimits,
) -> Result<WindowsProcess, WindowsVmiError> {
    if profile.symbols().contains_key("aegishv_current_eprocess") {
        let pointer = symbol_address(profile, "aegishv_current_eprocess", nt_base)?;
        let process = read_u64(memory, pointer)?;
        if process == 0 {
            return Err(inconsistent("current EPROCESS pointer is null"));
        }
        return read_process(profile, memory, process);
    }
    let cr3 = regs.cr3()?;
    let processes = walk_windows_processes(profile, memory, nt_base, limits)?;
    processes
        .into_iter()
        .find(|process| process.directory_table_base == Some(cr3))
        .ok_or_else(|| WindowsVmiError::InconsistentSnapshot {
            detail: format!("no EPROCESS directory table base matches CR3 0x{cr3:x}"),
        })
}

pub fn walk_windows_modules(
    profile: &WindowsProfile,
    memory: &dyn WindowsVirtualMemoryReader,
    nt_base: u64,
    limits: WindowsWalkLimits,
) -> Result<Vec<WindowsModule>, WindowsVmiError> {
    let head = symbol_address(profile, "PsLoadedModuleList", nt_base)?;
    let list_offset = field_offset(profile, "KLDR_DATA_TABLE_ENTRY", "InLoadOrderLinks")?;
    let mut modules = Vec::new();
    let mut visited = BTreeSet::new();
    let mut node = read_u64(memory, head)?;

    while node != head {
        if modules.len() >= limits.max_modules {
            return Err(inconsistent(format!(
                "loaded module list exceeded configured limit {}",
                limits.max_modules
            )));
        }
        if !visited.insert(node) {
            return Err(inconsistent(format!(
                "loaded module list looped before returning to head at 0x{node:x}"
            )));
        }
        if node < list_offset {
            return Err(inconsistent(format!(
                "module list node 0x{node:x} is smaller than list offset 0x{list_offset:x}"
            )));
        }
        let module_address = node - list_offset;
        modules.push(read_module(profile, memory, module_address)?);
        node = read_u64(memory, node)?;
    }

    Ok(modules)
}

pub fn windows_executable_ranges(
    profile: &WindowsProfile,
    modules: &[WindowsModule],
    nt_base: u64,
) -> Result<Vec<WindowsTextRange>, WindowsVmiError> {
    let mut ranges = Vec::new();
    if let Some(kernel) = profile.symbols().get("ntoskrnl.exe") {
        if let Some(size) = kernel.size {
            ranges.push(WindowsTextRange {
                owner: "ntoskrnl.exe".to_string(),
                start: nt_base + kernel.rva,
                end: nt_base + kernel.rva + size,
            });
        }
    }
    for module in modules {
        ranges.push(WindowsTextRange {
            owner: module.name.clone(),
            start: module.dll_base,
            end: module.dll_base.saturating_add(module.size_of_image),
        });
    }
    if ranges.is_empty() {
        return Err(WindowsVmiError::MissingProfileField {
            field: "ntoskrnl.exe text range or loaded module ranges".to_string(),
        });
    }
    Ok(ranges)
}

pub fn address_in_windows_text_ranges(
    address: u64,
    ranges: &[WindowsTextRange],
) -> Option<&WindowsTextRange> {
    ranges
        .iter()
        .find(|range| range.start <= address && address < range.end)
}

pub fn symbol_address(
    profile: &WindowsProfile,
    name: &str,
    nt_base: u64,
) -> Result<u64, WindowsVmiError> {
    let symbol =
        profile
            .symbols()
            .get(name)
            .ok_or_else(|| WindowsVmiError::MissingProfileField {
                field: format!("symbol:{name}"),
            })?;
    nt_base
        .checked_add(symbol.rva)
        .ok_or_else(|| inconsistent(format!("symbol '{name}' plus nt base overflows u64")))
}

pub fn read_u64(
    memory: &dyn WindowsVirtualMemoryReader,
    address: u64,
) -> Result<u64, WindowsVmiError> {
    let mut bytes = [0u8; 8];
    memory.read_virtual(address, &mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

pub fn read_u32(
    memory: &dyn WindowsVirtualMemoryReader,
    address: u64,
) -> Result<u32, WindowsVmiError> {
    let mut bytes = [0u8; 4];
    memory.read_virtual(address, &mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

pub fn read_c_string(
    memory: &dyn WindowsVirtualMemoryReader,
    address: u64,
    max_len: usize,
) -> Result<String, WindowsVmiError> {
    if max_len == 0 || max_len > 4096 {
        return Err(WindowsVmiError::Malformed {
            detail: format!("invalid bounded string length {max_len}"),
        });
    }
    let mut bytes = vec![0u8; max_len];
    memory.read_virtual(address, &mut bytes)?;
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8(bytes[..end].to_vec()).map_err(|_| WindowsVmiError::Malformed {
        detail: format!("guest string at 0x{address:x} is not UTF-8"),
    })
}

pub fn read_utf16_string(
    memory: &dyn WindowsVirtualMemoryReader,
    address: u64,
    max_bytes: usize,
) -> Result<String, WindowsVmiError> {
    if max_bytes == 0 || max_bytes > 4096 || max_bytes % 2 != 0 {
        return Err(WindowsVmiError::Malformed {
            detail: format!("invalid bounded UTF-16 byte length {max_bytes}"),
        });
    }
    let mut bytes = vec![0u8; max_bytes];
    memory.read_virtual(address, &mut bytes)?;
    let mut words = Vec::new();
    for chunk in bytes.chunks_exact(2) {
        let word = u16::from_le_bytes([chunk[0], chunk[1]]);
        if word == 0 {
            break;
        }
        words.push(word);
    }
    String::from_utf16(&words).map_err(|_| WindowsVmiError::Malformed {
        detail: format!("guest UTF-16 string at 0x{address:x} is invalid"),
    })
}

fn walk_process_list(
    profile: &WindowsProfile,
    memory: &dyn WindowsVirtualMemoryReader,
    head: u64,
    list_offset: u64,
    max_processes: usize,
) -> Result<Vec<WindowsProcess>, WindowsVmiError> {
    let system_process = head
        .checked_sub(list_offset)
        .ok_or_else(|| inconsistent("system process head is smaller than list offset"))?;
    let mut out = vec![read_process(profile, memory, system_process)?];
    let mut visited = BTreeSet::from([head]);
    let mut node = read_u64(memory, head)?;

    while node != head {
        if out.len() >= max_processes {
            return Err(inconsistent(format!(
                "process list exceeded configured limit {max_processes}"
            )));
        }
        if !visited.insert(node) {
            return Err(inconsistent(format!(
                "process list looped before returning to system process at 0x{node:x}"
            )));
        }
        if node < list_offset {
            return Err(inconsistent(format!(
                "process list node 0x{node:x} is smaller than list offset 0x{list_offset:x}"
            )));
        }
        out.push(read_process(profile, memory, node - list_offset)?);
        node = read_u64(memory, node)?;
    }

    Ok(out)
}

fn read_process(
    profile: &WindowsProfile,
    memory: &dyn WindowsVirtualMemoryReader,
    address: u64,
) -> Result<WindowsProcess, WindowsVmiError> {
    let pid = read_u64(
        memory,
        checked_add(
            address,
            field_offset(profile, "EPROCESS", "UniqueProcessId")?,
        )?,
    )?;
    let image_field = field(profile, "EPROCESS", "ImageFileName")?;
    let image_name = read_c_string(
        memory,
        checked_add(address, image_field.offset)?,
        field_size(image_field, 15)?,
    )?;
    let directory_table_base = optional_field_offset(profile, "EPROCESS", "DirectoryTableBase")
        .map(|offset| read_u64(memory, checked_add(address, offset)?))
        .transpose()?;
    Ok(WindowsProcess {
        address,
        pid,
        image_name,
        directory_table_base,
    })
}

fn read_module(
    profile: &WindowsProfile,
    memory: &dyn WindowsVirtualMemoryReader,
    address: u64,
) -> Result<WindowsModule, WindowsVmiError> {
    let name_field = field(profile, "KLDR_DATA_TABLE_ENTRY", "BaseDllName")?;
    let name = read_utf16_string(
        memory,
        checked_add(address, name_field.offset)?,
        field_size(name_field, 64)?,
    )?;
    let dll_base = read_u64(
        memory,
        checked_add(
            address,
            field_offset(profile, "KLDR_DATA_TABLE_ENTRY", "DllBase")?,
        )?,
    )?;
    let size_of_image = u64::from(read_u32(
        memory,
        checked_add(
            address,
            field_offset(profile, "KLDR_DATA_TABLE_ENTRY", "SizeOfImage")?,
        )?,
    )?);
    if size_of_image == 0 {
        return Err(inconsistent(format!(
            "loaded module '{name}' has an empty image range"
        )));
    }
    Ok(WindowsModule {
        address,
        name,
        dll_base,
        size_of_image,
    })
}

fn has_pe_header(
    memory: &dyn WindowsVirtualMemoryReader,
    base: u64,
) -> Result<bool, WindowsVmiError> {
    let mut mz = [0u8; 2];
    if memory.read_virtual(base, &mut mz).is_err() {
        return Ok(false);
    }
    if &mz != b"MZ" {
        return Ok(false);
    }
    let pe_offset = u64::from(read_u32(memory, checked_add(base, 0x3c)?)?);
    let pe_address = checked_add(base, pe_offset)?;
    let mut pe = [0u8; 4];
    memory.read_virtual(pe_address, &mut pe)?;
    Ok(&pe == b"PE\0\0")
}

fn field<'a>(
    profile: &'a WindowsProfile,
    struct_name: &str,
    field_name: &str,
) -> Result<&'a crate::windows_profile::WindowsStructOffset, WindowsVmiError> {
    let key = WindowsStructFieldKey {
        struct_name: struct_name.to_string(),
        field_name: field_name.to_string(),
    };
    profile
        .struct_offsets()
        .get(&key)
        .ok_or_else(|| WindowsVmiError::MissingProfileField {
            field: format!("{struct_name}.{field_name}"),
        })
}

fn field_offset(
    profile: &WindowsProfile,
    struct_name: &str,
    field_name: &str,
) -> Result<u64, WindowsVmiError> {
    Ok(field(profile, struct_name, field_name)?.offset)
}

fn optional_field_offset(
    profile: &WindowsProfile,
    struct_name: &str,
    field_name: &str,
) -> Option<u64> {
    let key = WindowsStructFieldKey {
        struct_name: struct_name.to_string(),
        field_name: field_name.to_string(),
    };
    profile.struct_offsets().get(&key).map(|field| field.offset)
}

fn field_size(
    field: &crate::windows_profile::WindowsStructOffset,
    fallback: usize,
) -> Result<usize, WindowsVmiError> {
    field
        .size
        .map(|size| {
            usize::try_from(size).map_err(|_| WindowsVmiError::Malformed {
                detail: format!(
                    "field '{}.{}' size does not fit this target",
                    field.struct_name, field.field_name
                ),
            })
        })
        .unwrap_or(Ok(fallback))
}

fn checked_add(base: u64, offset: u64) -> Result<u64, WindowsVmiError> {
    base.checked_add(offset)
        .ok_or_else(|| inconsistent("guest virtual address overflowed"))
}

fn inconsistent(detail: impl Into<String>) -> WindowsVmiError {
    WindowsVmiError::InconsistentSnapshot {
        detail: detail.into(),
    }
}
