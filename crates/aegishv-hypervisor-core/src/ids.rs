use crate::error::{CoreError, CoreErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VmId(u32);

impl VmId {
    pub const ONE: Self = Self(1);

    pub const fn new(raw: u32) -> Result<Self, CoreError> {
        if raw == 0 {
            Err(CoreError::new(
                CoreErrorKind::InvalidId,
                "vm id 0 is reserved",
            ))
        } else {
            Ok(Self(raw))
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VcpuId(u16);

impl VcpuId {
    pub const fn new(raw: u16) -> Result<Self, CoreError> {
        if raw == u16::MAX {
            Err(CoreError::new(
                CoreErrorKind::InvalidId,
                "vcpu id 65535 is reserved",
            ))
        } else {
            Ok(Self(raw))
        }
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysicalCpuId(u16);

impl PhysicalCpuId {
    pub const fn new(raw: u16) -> Result<Self, CoreError> {
        if raw == u16::MAX {
            Err(CoreError::new(
                CoreErrorKind::InvalidId,
                "physical cpu id 65535 is reserved",
            ))
        } else {
            Ok(Self(raw))
        }
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostPhysical(u64);

impl HostPhysical {
    pub const ZERO: Self = Self(0);

    pub const fn new(raw: u64) -> Result<Self, CoreError> {
        if raw == u64::MAX {
            Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "host physical address uses reserved sentinel value",
            ))
        } else {
            Ok(Self(raw))
        }
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn checked_add(self, value: u64) -> Result<Self, CoreError> {
        match self.0.checked_add(value) {
            Some(next) if next != u64::MAX => Ok(Self(next)),
            _ => Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "host physical address addition overflowed",
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GuestPhysical(u64);

impl GuestPhysical {
    pub const fn new(raw: u64) -> Result<Self, CoreError> {
        if raw == u64::MAX {
            Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "guest physical address uses reserved sentinel value",
            ))
        } else {
            Ok(Self(raw))
        }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GuestVirtual(u64);

impl GuestVirtual {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_reject_reserved_values() {
        assert_eq!(VmId::new(0).unwrap_err().kind, CoreErrorKind::InvalidId);
        assert_eq!(
            VcpuId::new(u16::MAX).unwrap_err().kind,
            CoreErrorKind::InvalidId
        );
        assert_eq!(
            PhysicalCpuId::new(u16::MAX).unwrap_err().kind,
            CoreErrorKind::InvalidId
        );
    }

    #[test]
    fn host_physical_address_detects_overflow() {
        let addr = HostPhysical::new(u64::MAX - 1).unwrap();

        assert_eq!(
            addr.checked_add(8).unwrap_err().kind,
            CoreErrorKind::InvalidAddress
        );
    }
}
