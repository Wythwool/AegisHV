use super::{TrapError, TrapErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PageSize {
    Size4K,
    Size2M,
    Size1G,
}

impl PageSize {
    pub fn bytes(self) -> u64 {
        match self {
            Self::Size4K => 4096,
            Self::Size2M => 2 * 1024 * 1024,
            Self::Size1G => 1024 * 1024 * 1024,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Size4K => "4k",
            Self::Size2M => "2m",
            Self::Size1G => "1g",
        }
    }

    pub fn is_aligned(self, addr: u64) -> bool {
        addr % self.bytes() == 0
    }

    pub fn align_down(self, addr: u64) -> u64 {
        addr - (addr % self.bytes())
    }

    pub fn immediate_child(self) -> Option<Self> {
        match self {
            Self::Size1G => Some(Self::Size2M),
            Self::Size2M => Some(Self::Size4K),
            Self::Size4K => None,
        }
    }

    pub fn split_count(self, target: Self) -> Result<u64, TrapError> {
        if self.immediate_child() != Some(target) {
            return Err(TrapError::new(
                TrapErrorKind::UnsupportedCapability,
                format!(
                    "stage-2 model splits one page-table level at a time; cannot split {} directly to {}",
                    self.as_str(),
                    target.as_str()
                ),
            ));
        }
        Ok(self.bytes() / target.bytes())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage2BackendKind {
    Synthetic,
    IntelEpt,
    AmdNpt,
    ArmStage2,
}

impl Stage2BackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Synthetic => "synthetic",
            Self::IntelEpt => "intel_ept",
            Self::AmdNpt => "amd_npt",
            Self::ArmStage2 => "arm_stage2",
        }
    }

    pub fn limits(self) -> Stage2Limits {
        match self {
            Self::Synthetic => Stage2Limits {
                backend: self,
                page_sizes: vec![PageSize::Size4K, PageSize::Size2M, PageSize::Size1G],
                memory_types: vec![MemoryType::WriteBack, MemoryType::Uncacheable],
                supports_execute_only: true,
                supports_write_without_read: true,
                note: "pure model; no hardware permission writes",
            },
            Self::IntelEpt => Stage2Limits {
                backend: self,
                page_sizes: vec![PageSize::Size4K, PageSize::Size2M, PageSize::Size1G],
                memory_types: vec![
                    MemoryType::Uncacheable,
                    MemoryType::WriteCombining,
                    MemoryType::WriteThrough,
                    MemoryType::WriteProtected,
                    MemoryType::WriteBack,
                ],
                supports_execute_only: true,
                supports_write_without_read: true,
                note: "model of EPT permission bits and memory type field",
            },
            Self::AmdNpt => Stage2Limits {
                backend: self,
                page_sizes: vec![PageSize::Size4K, PageSize::Size2M, PageSize::Size1G],
                memory_types: vec![MemoryType::WriteBack, MemoryType::Uncacheable],
                supports_execute_only: false,
                supports_write_without_read: false,
                note: "model of NPT R/W/NX semantics",
            },
            Self::ArmStage2 => Stage2Limits {
                backend: self,
                page_sizes: vec![PageSize::Size4K, PageSize::Size2M, PageSize::Size1G],
                memory_types: vec![MemoryType::Normal, MemoryType::Device],
                supports_execute_only: true,
                supports_write_without_read: false,
                note: "model of Stage-2 access permissions and XN bits",
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage2Limits {
    pub backend: Stage2BackendKind,
    pub page_sizes: Vec<PageSize>,
    pub memory_types: Vec<MemoryType>,
    pub supports_execute_only: bool,
    pub supports_write_without_read: bool,
    pub note: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MemoryType {
    WriteBack,
    WriteThrough,
    WriteCombining,
    WriteProtected,
    Uncacheable,
    Normal,
    Device,
}

impl MemoryType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WriteBack => "write_back",
            Self::WriteThrough => "write_through",
            Self::WriteCombining => "write_combining",
            Self::WriteProtected => "write_protected",
            Self::Uncacheable => "uncacheable",
            Self::Normal => "normal",
            Self::Device => "device",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Stage2Permissions {
    pub read: bool,
    pub write: bool,
    pub exec: bool,
}

impl Stage2Permissions {
    pub const NONE: Self = Self::new(false, false, false);
    pub const READ: Self = Self::new(true, false, false);
    pub const READ_WRITE: Self = Self::new(true, true, false);
    pub const READ_EXEC: Self = Self::new(true, false, true);
    pub const EXEC: Self = Self::new(false, false, true);
    pub const RWX: Self = Self::new(true, true, true);

    pub const fn new(read: bool, write: bool, exec: bool) -> Self {
        Self { read, write, exec }
    }

    pub fn without_access(self, access: TrapAccessKind) -> Self {
        match access {
            TrapAccessKind::Read => Self {
                read: false,
                ..self
            },
            TrapAccessKind::Write => Self {
                write: false,
                ..self
            },
            TrapAccessKind::Execute => Self {
                exec: false,
                ..self
            },
        }
    }

    pub fn with_access(self, access: TrapAccessKind, enabled: bool) -> Self {
        match access {
            TrapAccessKind::Read => Self {
                read: enabled,
                ..self
            },
            TrapAccessKind::Write => Self {
                write: enabled,
                ..self
            },
            TrapAccessKind::Execute => Self {
                exec: enabled,
                ..self
            },
        }
    }

    pub fn allows(self, access: TrapAccessKind) -> bool {
        match access {
            TrapAccessKind::Read => self.read,
            TrapAccessKind::Write => self.write,
            TrapAccessKind::Execute => self.exec,
        }
    }

    pub fn is_wx(self) -> bool {
        self.write && self.exec
    }

    pub fn compact(self) -> String {
        let mut out = String::with_capacity(3);
        out.push(if self.read { 'r' } else { '-' });
        out.push(if self.write { 'w' } else { '-' });
        out.push(if self.exec { 'x' } else { '-' });
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrapAccessKind {
    Read,
    Write,
    Execute,
}

impl TrapAccessKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Execute => "execute",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage2Mapping {
    pub owner_vm: String,
    pub address_space: String,
    pub base: u64,
    pub page_size: PageSize,
    pub memory_type: MemoryType,
    pub permissions: Stage2Permissions,
}

impl Stage2Mapping {
    pub fn new(
        owner_vm: impl Into<String>,
        address_space: impl Into<String>,
        base: u64,
        page_size: PageSize,
        memory_type: MemoryType,
        permissions: Stage2Permissions,
    ) -> Result<Self, TrapError> {
        if !page_size.is_aligned(base) {
            return Err(TrapError::new(
                TrapErrorKind::Misaligned,
                format!(
                    "stage-2 mapping base {base:#x} is not aligned to {}",
                    page_size.as_str()
                ),
            ));
        }
        let owner_vm = owner_vm.into();
        if owner_vm.trim().is_empty() {
            return Err(TrapError::new(
                TrapErrorKind::MalformedInput,
                "stage-2 mapping requires a non-empty owner VM",
            ));
        }
        let address_space = address_space.into();
        if address_space.trim().is_empty() {
            return Err(TrapError::new(
                TrapErrorKind::MalformedInput,
                "stage-2 mapping requires a non-empty address space",
            ));
        }
        Ok(Self {
            owner_vm,
            address_space,
            base,
            page_size,
            memory_type,
            permissions,
        })
    }

    pub fn end(&self) -> Result<u64, TrapError> {
        self.base
            .checked_add(self.page_size.bytes())
            .ok_or_else(|| {
                TrapError::new(
                    TrapErrorKind::InvalidAddress,
                    format!(
                        "stage-2 mapping at {:#x} overflows address space",
                        self.base
                    ),
                )
            })
    }

    pub fn contains(&self, gpa: u64) -> bool {
        self.end()
            .map(|end| self.base <= gpa && gpa < end)
            .unwrap_or(false)
    }
}
