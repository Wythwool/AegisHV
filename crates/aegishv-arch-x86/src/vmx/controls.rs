use super::features::{VmxError, VmxErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VmxControlGroup {
    PinBased,
    PrimaryProcessor,
    SecondaryProcessor,
    Exit,
    Entry,
}

impl VmxControlGroup {
    pub const fn name(self) -> &'static str {
        match self {
            Self::PinBased => "pin-based VM-execution controls",
            Self::PrimaryProcessor => "primary processor-based VM-execution controls",
            Self::SecondaryProcessor => "secondary processor-based VM-execution controls",
            Self::Exit => "VM-exit controls",
            Self::Entry => "VM-entry controls",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmxControlMsr {
    pub group: VmxControlGroup,
    pub must_be_one: u32,
    pub allowed_one: u32,
}

impl VmxControlMsr {
    pub const fn from_raw(group: VmxControlGroup, raw: u64) -> Self {
        Self {
            group,
            must_be_one: raw as u32,
            allowed_one: (raw >> 32) as u32,
        }
    }

    pub const fn new(group: VmxControlGroup, must_be_one: u32, allowed_one: u32) -> Self {
        Self {
            group,
            must_be_one,
            allowed_one,
        }
    }

    pub const fn adjust(self, requested: u32) -> Result<u32, VmxError> {
        let adjusted = requested | self.must_be_one;
        if adjusted & !self.allowed_one != 0 {
            Err(VmxError::new(
                VmxErrorKind::InvalidControlBits,
                "VMX control request sets a bit that the CPU does not allow",
            ))
        } else {
            Ok(adjusted)
        }
    }
}

pub const PIN_BASED_EXT_INTR_EXITING: u32 = 1 << 0;
pub const PIN_BASED_NMI_EXITING: u32 = 1 << 3;

pub const PRIMARY_HLT_EXITING: u32 = 1 << 7;
pub const PRIMARY_CR3_LOAD_EXITING: u32 = 1 << 15;
pub const PRIMARY_CR3_STORE_EXITING: u32 = 1 << 16;
pub const PRIMARY_USE_MSR_BITMAPS: u32 = 1 << 28;
pub const PRIMARY_ACTIVATE_SECONDARY_CONTROLS: u32 = 1 << 31;

pub const SECONDARY_ENABLE_EPT: u32 = 1 << 1;
pub const SECONDARY_ENABLE_VPID: u32 = 1 << 5;
pub const SECONDARY_ENABLE_RDTSCP: u32 = 1 << 3;
pub const SECONDARY_ENABLE_INVPCID: u32 = 1 << 12;
pub const SECONDARY_MONITOR_TRAP_FLAG: u32 = 1 << 27;

pub const EXIT_HOST_ADDRESS_SPACE_SIZE: u32 = 1 << 9;
pub const EXIT_SAVE_IA32_EFER: u32 = 1 << 20;
pub const EXIT_LOAD_IA32_EFER: u32 = 1 << 21;
pub const ENTRY_IA32E_MODE_GUEST: u32 = 1 << 9;
pub const ENTRY_LOAD_IA32_EFER: u32 = 1 << 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmxControlRequest {
    pub pin_based: u32,
    pub primary: u32,
    pub secondary: u32,
    pub exit: u32,
    pub entry: u32,
}

impl VmxControlRequest {
    pub const fn toy_hlt_guest() -> Self {
        Self {
            pin_based: PIN_BASED_NMI_EXITING,
            primary: PRIMARY_HLT_EXITING | PRIMARY_ACTIVATE_SECONDARY_CONTROLS,
            secondary: SECONDARY_ENABLE_EPT,
            exit: EXIT_HOST_ADDRESS_SPACE_SIZE | EXIT_SAVE_IA32_EFER | EXIT_LOAD_IA32_EFER,
            entry: ENTRY_IA32E_MODE_GUEST | ENTRY_LOAD_IA32_EFER,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmxControlMsrs {
    pub pin_based: VmxControlMsr,
    pub primary: VmxControlMsr,
    pub secondary: VmxControlMsr,
    pub exit: VmxControlMsr,
    pub entry: VmxControlMsr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmxControlFields {
    pub pin_based: u32,
    pub primary: u32,
    pub secondary: u32,
    pub exit: u32,
    pub entry: u32,
}

impl VmxControlMsrs {
    pub fn build(self, request: VmxControlRequest) -> Result<VmxControlFields, VmxError> {
        let fields = VmxControlFields {
            pin_based: self.pin_based.adjust(request.pin_based)?,
            primary: self.primary.adjust(request.primary)?,
            secondary: self.secondary.adjust(request.secondary)?,
            exit: self.exit.adjust(request.exit)?,
            entry: self.entry.adjust(request.entry)?,
        };
        let supported_pin = PIN_BASED_NMI_EXITING;
        let supported_primary = PRIMARY_HLT_EXITING | PRIMARY_ACTIVATE_SECONDARY_CONTROLS;
        let supported_secondary = SECONDARY_ENABLE_EPT;
        let supported_exit =
            EXIT_HOST_ADDRESS_SPACE_SIZE | EXIT_SAVE_IA32_EFER | EXIT_LOAD_IA32_EFER;
        let supported_entry = ENTRY_IA32E_MODE_GUEST | ENTRY_LOAD_IA32_EFER;
        if fields.pin_based & !supported_pin != 0
            || fields.primary & !supported_primary != 0
            || fields.secondary & !supported_secondary != 0
            || fields.exit & !supported_exit != 0
            || fields.entry & !supported_entry != 0
        {
            return Err(VmxError::new(
                VmxErrorKind::InvalidControlBits,
                "CPU requires a VMX control that the toy guest runtime does not implement",
            ));
        }
        Ok(fields)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permissive_msrs() -> VmxControlMsrs {
        VmxControlMsrs {
            pin_based: VmxControlMsr::new(VmxControlGroup::PinBased, 0, u32::MAX),
            primary: VmxControlMsr::new(VmxControlGroup::PrimaryProcessor, 0, u32::MAX),
            secondary: VmxControlMsr::new(VmxControlGroup::SecondaryProcessor, 0, u32::MAX),
            exit: VmxControlMsr::new(VmxControlGroup::Exit, 0, u32::MAX),
            entry: VmxControlMsr::new(VmxControlGroup::Entry, 0, u32::MAX),
        }
    }

    #[test]
    fn control_builder_forces_must_be_one_bits() {
        let msr = VmxControlMsr::new(
            VmxControlGroup::PrimaryProcessor,
            PRIMARY_HLT_EXITING,
            u32::MAX,
        );

        assert_eq!(msr.adjust(0).unwrap(), PRIMARY_HLT_EXITING);
    }

    #[test]
    fn control_builder_rejects_forbidden_one_bits() {
        let msr = VmxControlMsr::new(VmxControlGroup::SecondaryProcessor, 0, SECONDARY_ENABLE_EPT);
        let err = msr.adjust(SECONDARY_ENABLE_VPID).unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::InvalidControlBits);
    }

    #[test]
    fn toy_hlt_guest_request_enables_required_controls() {
        let fields = permissive_msrs()
            .build(VmxControlRequest::toy_hlt_guest())
            .unwrap();

        assert_ne!(fields.primary & PRIMARY_HLT_EXITING, 0);
        assert_ne!(fields.pin_based & PIN_BASED_NMI_EXITING, 0);
        assert_ne!(fields.secondary & SECONDARY_ENABLE_EPT, 0);
        assert_eq!(fields.secondary & SECONDARY_ENABLE_VPID, 0);
        assert_ne!(fields.exit & EXIT_HOST_ADDRESS_SPACE_SIZE, 0);
        assert_ne!(fields.exit & EXIT_SAVE_IA32_EFER, 0);
        assert_ne!(fields.exit & EXIT_LOAD_IA32_EFER, 0);
        assert_ne!(fields.entry & ENTRY_LOAD_IA32_EFER, 0);
    }

    #[test]
    fn toy_hlt_guest_rejects_forced_controls_without_runtime_support() {
        let mut msrs = permissive_msrs();
        msrs.primary.must_be_one = PRIMARY_USE_MSR_BITMAPS;

        let error = msrs.build(VmxControlRequest::toy_hlt_guest()).unwrap_err();

        assert_eq!(error.kind, VmxErrorKind::InvalidControlBits);
    }
}
