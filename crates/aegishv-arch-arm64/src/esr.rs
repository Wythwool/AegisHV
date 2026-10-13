use aegishv_hypervisor_core::ids::GuestPhysical;

use crate::features::{Arm64Error, Arm64ErrorKind};
use crate::stage2::Stage2Access;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExceptionClass {
    WfiWfe,
    Hvc64,
    Smc64,
    InstructionAbortLowerEl,
    DataAbortLowerEl,
    Unknown(u8),
}

impl ExceptionClass {
    pub const fn from_raw(raw: u8) -> Self {
        match raw {
            0x01 => Self::WfiWfe,
            0x16 => Self::Hvc64,
            0x17 => Self::Smc64,
            0x20 => Self::InstructionAbortLowerEl,
            0x24 => Self::DataAbortLowerEl,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EsrEl2 {
    raw: u64,
}

impl EsrEl2 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    pub const fn exception_class(self) -> ExceptionClass {
        ExceptionClass::from_raw(((self.raw >> 26) & 0x3f) as u8)
    }

    pub const fn iss(self) -> u32 {
        (self.raw & 0x01ff_ffff) as u32
    }

    pub const fn il(self) -> bool {
        self.raw & (1 << 25) != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultStatusCode {
    AddressSize { level: u8 },
    Translation { level: u8 },
    AccessFlag { level: u8 },
    Permission { level: u8 },
    External,
    Other(u8),
}

impl FaultStatusCode {
    pub const fn decode(raw: u8) -> Self {
        let level = raw & 0x3;
        match raw {
            0b000000..=0b000011 => Self::AddressSize { level },
            0b000100..=0b000111 => Self::Translation { level },
            0b001000..=0b001011 => Self::AccessFlag { level },
            0b001100..=0b001111 => Self::Permission { level },
            0b010000 => Self::External,
            other => Self::Other(other),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stage2Fault {
    pub access: Stage2Access,
    pub status: FaultStatusCode,
    pub far_valid: bool,
    pub far_el2: u64,
    pub hpfar_el2: u64,
    pub ipa: Option<GuestPhysical>,
    pub stage1_page_table_walk: bool,
}

impl Stage2Fault {
    pub fn decode(esr: EsrEl2, far_el2: u64, hpfar_el2: u64) -> Result<Self, Arm64Error> {
        match esr.exception_class() {
            ExceptionClass::InstructionAbortLowerEl | ExceptionClass::DataAbortLowerEl => {}
            _ => {
                return Err(Arm64Error::new(
                    Arm64ErrorKind::InvalidEsr,
                    "ESR_EL2 is not an abort from a lower exception level",
                ))
            }
        }

        let iss = esr.iss();
        let far_valid = iss & (1 << 10) == 0;
        let stage1_page_table_walk = iss & (1 << 7) != 0;
        let status = FaultStatusCode::decode((iss & 0x3f) as u8);
        let access = match esr.exception_class() {
            ExceptionClass::InstructionAbortLowerEl => Stage2Access::Execute,
            ExceptionClass::DataAbortLowerEl if iss & (1 << 6) != 0 => Stage2Access::Write,
            ExceptionClass::DataAbortLowerEl => Stage2Access::Read,
            _ => Stage2Access::Read,
        };
        let ipa = if far_valid {
            let ipa = ((hpfar_el2 & 0x0fff_ffff) << 8) | (far_el2 & 0xfff);
            Some(GuestPhysical::new(ipa).map_err(|_| {
                Arm64Error::new(
                    Arm64ErrorKind::InvalidEsr,
                    "decoded ARM64 IPA used a reserved sentinel value",
                )
            })?)
        } else {
            None
        };

        Ok(Self {
            access,
            status,
            far_valid,
            far_el2,
            hpfar_el2,
            ipa,
            stage1_page_table_walk,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EC_DABT_LOWER: u64 = 0x24 << 26;
    const EC_IABT_LOWER: u64 = 0x20 << 26;

    #[test]
    fn data_abort_decode_uses_hpfar_and_far_page_offset_for_ipa() {
        let esr = EsrEl2::new(EC_DABT_LOWER | (1 << 6) | 0b001101);
        let fault = Stage2Fault::decode(esr, 0xabc, 0x12345).unwrap();

        assert_eq!(fault.access, Stage2Access::Write);
        assert_eq!(fault.ipa.unwrap().get(), (0x12345 << 8) | 0xabc);
        assert_eq!(fault.status, FaultStatusCode::Permission { level: 1 });
    }

    #[test]
    fn instruction_abort_is_execute_and_honors_fnv() {
        let esr = EsrEl2::new(EC_IABT_LOWER | (1 << 10) | 0b000100);
        let fault = Stage2Fault::decode(esr, 0x1000, 0x20).unwrap();

        assert_eq!(fault.access, Stage2Access::Execute);
        assert!(!fault.far_valid);
        assert_eq!(fault.ipa, None);
    }

    #[test]
    fn stage1_walk_bit_is_preserved() {
        let esr = EsrEl2::new(EC_DABT_LOWER | (1 << 7) | 0b000100);
        let fault = Stage2Fault::decode(esr, 0, 0).unwrap();

        assert!(fault.stage1_page_table_walk);
    }
}
