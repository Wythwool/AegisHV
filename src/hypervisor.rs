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
    pub stage2_permissions: bool,
    pub tlb_invalidation: bool,
    pub single_step: bool,
    pub huge_page_split: bool,
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
            stage2_permissions: false,
            tlb_invalidation: false,
            single_step: false,
            huge_page_split: false,
            syscall_path_checks: false,
            grouped_pmu_sampling: false,
        }
    }
    fn health(&self) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrapEnforcementRequirements {
    pub permission_traps: bool,
    pub stage2_permissions: bool,
    pub tlb_invalidation: bool,
    pub single_step: bool,
    pub huge_page_split: bool,
}

impl Default for TrapEnforcementRequirements {
    fn default() -> Self {
        Self {
            permission_traps: true,
            stage2_permissions: true,
            tlb_invalidation: true,
            single_step: true,
            huge_page_split: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrapNegotiation {
    pub backend: &'static str,
    pub mode: BackendMode,
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrapNegotiationError {
    pub backend: &'static str,
    pub missing: Vec<&'static str>,
}

impl std::fmt::Display for TrapNegotiationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "backend {} cannot enforce traps; missing capabilities: {}",
            self.backend,
            self.missing.join(",")
        )
    }
}

impl std::error::Error for TrapNegotiationError {}

pub fn negotiate_trap_enforcement(
    backend: &dyn HypervisorBackend,
    requirements: TrapEnforcementRequirements,
) -> Result<TrapNegotiation, TrapNegotiationError> {
    let caps = backend.capabilities();
    let mut missing = Vec::new();
    if requirements.permission_traps && !caps.permission_traps {
        missing.push("permission_traps");
    }
    if requirements.stage2_permissions && !caps.stage2_permissions {
        missing.push("stage2_permissions");
    }
    if requirements.tlb_invalidation && !caps.tlb_invalidation {
        missing.push("tlb_invalidation");
    }
    if requirements.single_step && !caps.single_step {
        missing.push("single_step");
    }
    if requirements.huge_page_split && !caps.huge_page_split {
        missing.push("huge_page_split");
    }
    if !missing.is_empty() {
        return Err(TrapNegotiationError {
            backend: backend.name(),
            missing,
        });
    }
    Ok(TrapNegotiation {
        backend: backend.name(),
        mode: caps.mode,
        accepted: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_backend_does_not_claim_vmx() {
        let b = NoHypervisorBackend;
        assert_eq!(b.capabilities().arch, BackendArch::None);
    }

    #[test]
    fn no_backend_refuses_trap_enforcement_negotiation() {
        let b = NoHypervisorBackend;
        let err = negotiate_trap_enforcement(&b, TrapEnforcementRequirements::default())
            .expect_err("host-side sensor must not accept trap enforcement");

        assert!(err.missing.contains(&"permission_traps"));
        assert!(err.missing.contains(&"tlb_invalidation"));
        assert!(err.missing.contains(&"single_step"));
        assert!(err.missing.contains(&"huge_page_split"));
    }
}
