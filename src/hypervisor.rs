#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendArch {
    None,
    IntelVmx,
    AmdSvm,
    Arm64El2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendMode {
    HostSideSensor,
    KvmVmi,
    TrapEnforcement,
    Type1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub arch: BackendArch,
    pub mode: BackendMode,
    pub memory_read: bool,
    pub vcpu_registers: bool,
    pub gva_translation: bool,
    pub permission_traps: bool,
    pub syscall_path_checks: bool,
    pub grouped_pmu_sampling: bool,
}

pub trait HypervisorBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> BackendCapabilities;
    fn health(&self) -> Result<(), String>;
}

pub struct NoHypervisorBackend;

impl HypervisorBackend for NoHypervisorBackend {
    fn name(&self) -> &'static str {
        "host-side-sensor"
    }
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            arch: BackendArch::None,
            mode: BackendMode::HostSideSensor,
            memory_read: false,
            vcpu_registers: false,
            gva_translation: false,
            permission_traps: false,
            syscall_path_checks: false,
            grouped_pmu_sampling: false,
        }
    }
    fn health(&self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_backend_does_not_claim_vmx() {
        let b = NoHypervisorBackend;
        assert_eq!(b.capabilities().arch, BackendArch::None);
    }
}
