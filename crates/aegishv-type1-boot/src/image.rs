#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootImageFormat {
    KernelElf,
    LimineIso,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QemuEvidencePlan<'a> {
    pub machine: &'a str,
    pub cpu: &'a str,
    pub memory_mib: u16,
    pub serial_log_path: &'a str,
    pub expected_serial_marker: &'a str,
    pub timeout_seconds: u16,
}

impl<'a> QemuEvidencePlan<'a> {
    pub const fn x86_64_limine_default() -> Self {
        Self {
            machine: "q35",
            cpu: "qemu64",
            memory_mib: 256,
            serial_log_path: "target/type1/qemu-serial.log",
            expected_serial_marker: "aegishv:type1:halt",
            timeout_seconds: 15,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootImagePlan<'a> {
    pub format: BootImageFormat,
    pub kernel_elf: &'a str,
    pub limine_config: &'a str,
    pub linker_script: &'a str,
    pub entry_stub: &'a str,
    pub output_image: &'a str,
    pub qemu: QemuEvidencePlan<'a>,
}

impl<'a> BootImagePlan<'a> {
    pub const fn x86_64_limine_lab() -> Self {
        Self {
            format: BootImageFormat::LimineIso,
            kernel_elf: "target/type1/aegishv-type1.elf",
            limine_config: "boot/limine/limine.conf",
            linker_script: "boot/linker/x86_64-type1.ld",
            entry_stub: "boot/x86_64/entry.S",
            output_image: "target/type1/aegishv-type1.iso",
            qemu: QemuEvidencePlan::x86_64_limine_default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootImagePlanError {
    KernelElfPathMissing,
    KernelElfExtensionInvalid,
    LimineConfigPathMissing,
    LimineConfigExtensionInvalid,
    LinkerScriptPathMissing,
    LinkerScriptExtensionInvalid,
    EntryStubPathMissing,
    EntryStubExtensionInvalid,
    OutputImagePathMissing,
    OutputImageExtensionInvalid,
    QemuMachineMissing,
    QemuCpuMissing,
    QemuMemoryTooSmall,
    QemuSerialLogMissing,
    QemuSerialMarkerMissing,
    QemuTimeoutTooShort,
}

pub fn validate_boot_image_plan(plan: BootImagePlan<'_>) -> Result<(), BootImagePlanError> {
    validate_required_path(plan.kernel_elf, BootImagePlanError::KernelElfPathMissing)?;
    validate_suffix(
        plan.kernel_elf,
        ".elf",
        BootImagePlanError::KernelElfExtensionInvalid,
    )?;

    validate_required_path(
        plan.limine_config,
        BootImagePlanError::LimineConfigPathMissing,
    )?;
    validate_suffix(
        plan.limine_config,
        ".conf",
        BootImagePlanError::LimineConfigExtensionInvalid,
    )?;

    validate_required_path(
        plan.linker_script,
        BootImagePlanError::LinkerScriptPathMissing,
    )?;
    validate_suffix(
        plan.linker_script,
        ".ld",
        BootImagePlanError::LinkerScriptExtensionInvalid,
    )?;

    validate_required_path(plan.entry_stub, BootImagePlanError::EntryStubPathMissing)?;
    if !(plan.entry_stub.ends_with(".S") || plan.entry_stub.ends_with(".s")) {
        return Err(BootImagePlanError::EntryStubExtensionInvalid);
    }

    validate_required_path(
        plan.output_image,
        BootImagePlanError::OutputImagePathMissing,
    )?;
    match plan.format {
        BootImageFormat::KernelElf => validate_suffix(
            plan.output_image,
            ".elf",
            BootImagePlanError::OutputImageExtensionInvalid,
        )?,
        BootImageFormat::LimineIso => validate_suffix(
            plan.output_image,
            ".iso",
            BootImagePlanError::OutputImageExtensionInvalid,
        )?,
    }

    validate_required_path(plan.qemu.machine, BootImagePlanError::QemuMachineMissing)?;
    validate_required_path(plan.qemu.cpu, BootImagePlanError::QemuCpuMissing)?;
    if plan.qemu.memory_mib < 128 {
        return Err(BootImagePlanError::QemuMemoryTooSmall);
    }
    validate_required_path(
        plan.qemu.serial_log_path,
        BootImagePlanError::QemuSerialLogMissing,
    )?;
    validate_required_path(
        plan.qemu.expected_serial_marker,
        BootImagePlanError::QemuSerialMarkerMissing,
    )?;
    if plan.qemu.timeout_seconds < 5 {
        return Err(BootImagePlanError::QemuTimeoutTooShort);
    }

    Ok(())
}

fn validate_required_path(path: &str, error: BootImagePlanError) -> Result<(), BootImagePlanError> {
    if path.trim().is_empty() || path.as_bytes().contains(&0) {
        return Err(error);
    }
    Ok(())
}

fn validate_suffix(
    path: &str,
    suffix: &str,
    error: BootImagePlanError,
) -> Result<(), BootImagePlanError> {
    if path.ends_with(suffix) {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x86_64_limine_image_plan_records_artifact_contract() {
        let plan = BootImagePlan::x86_64_limine_lab();

        validate_boot_image_plan(plan).unwrap();
        assert_eq!(plan.format, BootImageFormat::LimineIso);
        assert_eq!(plan.qemu.machine, "q35");
        assert_eq!(plan.qemu.expected_serial_marker, "aegishv:type1:halt");
    }

    #[test]
    fn image_plan_rejects_bad_paths_and_weak_qemu_gate() {
        let mut plan = BootImagePlan::x86_64_limine_lab();
        plan.kernel_elf = "target/type1/aegishv-type1.bin";
        assert_eq!(
            validate_boot_image_plan(plan).unwrap_err(),
            BootImagePlanError::KernelElfExtensionInvalid
        );

        let mut plan = BootImagePlan::x86_64_limine_lab();
        plan.qemu.expected_serial_marker = "";
        assert_eq!(
            validate_boot_image_plan(plan).unwrap_err(),
            BootImagePlanError::QemuSerialMarkerMissing
        );

        let mut plan = BootImagePlan::x86_64_limine_lab();
        plan.qemu.timeout_seconds = 4;
        assert_eq!(
            validate_boot_image_plan(plan).unwrap_err(),
            BootImagePlanError::QemuTimeoutTooShort
        );
    }
}
