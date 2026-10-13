mod error;
mod memory;

pub use error::{
    BackendError, MemoryReadError, ProfileError, RegisterReadError, TranslationError, VmiErrorKind,
};
pub(crate) use memory::read_table_entry;
pub use memory::SyntheticGuestPhysicalMemoryReader;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VmId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VcpuId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GuestPhysical(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GuestVirtual(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestRegisters {
    pub pc: u64,
    pub sp: u64,
    pub cr3_or_ttbr: Option<u64>,
    pub privilege: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationResult {
    pub gpa: GuestPhysical,
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
    pub user: bool,
    pub page_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyscallPathReport {
    pub ok: bool,
    pub os: String,
    pub entry: Option<u64>,
    pub table: Option<u64>,
    pub findings: Vec<String>,
}

pub trait GuestMemoryReader: Send + Sync {
    fn read_physical(
        &self,
        vm: VmId,
        gpa: GuestPhysical,
        buf: &mut [u8],
    ) -> Result<usize, MemoryReadError>;
}

pub trait VcpuRegisterReader: Send + Sync {
    fn read_registers(&self, vm: VmId, vcpu: VcpuId) -> Result<GuestRegisters, RegisterReadError>;
}

pub trait AddressTranslator: Send + Sync {
    fn translate(
        &self,
        vm: VmId,
        regs: &GuestRegisters,
        gva: GuestVirtual,
    ) -> Result<TranslationResult, TranslationError>;
}
