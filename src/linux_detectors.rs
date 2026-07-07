use std::collections::BTreeMap;

use crate::linux_integrity::{
    check_linux_kernel_text_hash, check_linux_module_text_hashes, LinuxIntegrityReport,
};
use crate::linux_syscall::{
    inspect_linux_lstar, inspect_linux_syscall_table, linux_syscall_path_report, LinuxLstarReport,
    LinuxSyscallTableReport,
};
use crate::linux_vmi::{LinuxTextRange, LinuxVirtualMemoryReader, LinuxVmiError};
use crate::linux_x86::{inspect_linux_control_registers, LinuxControlPolicy, LinuxControlReport};
use crate::vmi::SyscallPathReport;
use crate::vmi_linux_profile::LinuxProfile;
use crate::vmi_registers::X86_64RegisterSnapshot;

pub struct LinuxDetectorInputs<'a> {
    pub profile: &'a LinuxProfile,
    pub memory: &'a dyn LinuxVirtualMemoryReader,
    pub registers: &'a X86_64RegisterSnapshot,
    pub slide: u64,
    pub executable_ranges: &'a [LinuxTextRange],
    pub kernel_text_baselines: &'a BTreeMap<String, String>,
    pub module_text_baselines: &'a BTreeMap<String, String>,
    pub control_policy: LinuxControlPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxDetectorCheck {
    pub name: &'static str,
    pub ok: bool,
    pub finding_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxDetectorFinding {
    pub check: &'static str,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxDetectorRun {
    pub ok: bool,
    pub checks: Vec<LinuxDetectorCheck>,
    pub findings: Vec<LinuxDetectorFinding>,
    pub syscall_table: LinuxSyscallTableReport,
    pub lstar: LinuxLstarReport,
    pub syscall_path: SyscallPathReport,
    pub controls: LinuxControlReport,
    pub kernel_text: LinuxIntegrityReport,
    pub module_text: LinuxIntegrityReport,
}

pub fn run_linux_detector_checks(
    inputs: LinuxDetectorInputs<'_>,
) -> Result<LinuxDetectorRun, LinuxVmiError> {
    let syscall_table = inspect_linux_syscall_table(
        inputs.profile,
        inputs.memory,
        inputs.slide,
        inputs.executable_ranges,
    )?;
    let lstar = inspect_linux_lstar(
        inputs.profile,
        inputs.registers,
        inputs.slide,
        inputs.executable_ranges,
    )?;
    let syscall_path = linux_syscall_path_report(inputs.profile, &syscall_table, &lstar);
    let controls = inspect_linux_control_registers(inputs.registers, inputs.control_policy)?;
    let kernel_text = check_linux_kernel_text_hash(
        inputs.memory,
        inputs.executable_ranges,
        inputs.kernel_text_baselines,
    )?;
    let module_text = check_linux_module_text_hashes(
        inputs.memory,
        inputs.executable_ranges,
        inputs.module_text_baselines,
    )?;

    let mut findings = Vec::new();
    add_findings(&mut findings, "lstar", &lstar.findings);
    add_findings(&mut findings, "syscall_table", &syscall_table.findings);
    add_findings(&mut findings, "control_registers", &controls.findings);
    add_findings(&mut findings, "kernel_text_hash", &kernel_text.findings);
    add_findings(&mut findings, "module_text_hash", &module_text.findings);

    let checks = vec![
        check("lstar", lstar.ok, lstar.findings.len()),
        check(
            "syscall_table",
            syscall_table.ok,
            syscall_table.findings.len(),
        ),
        check("control_registers", controls.ok, controls.findings.len()),
        check(
            "kernel_text_hash",
            kernel_text.ok,
            kernel_text.findings.len(),
        ),
        check(
            "module_text_hash",
            module_text.ok,
            module_text.findings.len(),
        ),
    ];

    Ok(LinuxDetectorRun {
        ok: findings.is_empty(),
        checks,
        findings,
        syscall_table,
        lstar,
        syscall_path,
        controls,
        kernel_text,
        module_text,
    })
}

fn check(name: &'static str, ok: bool, finding_count: usize) -> LinuxDetectorCheck {
    LinuxDetectorCheck {
        name,
        ok,
        finding_count,
    }
}

fn add_findings(out: &mut Vec<LinuxDetectorFinding>, check: &'static str, findings: &[String]) {
    out.extend(findings.iter().map(|detail| LinuxDetectorFinding {
        check,
        detail: detail.clone(),
    }));
}
