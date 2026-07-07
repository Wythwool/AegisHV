use std::collections::BTreeMap;

use crate::vmi::SyscallPathReport;
use crate::vmi_registers::X86_64RegisterSnapshot;
use crate::windows_callbacks::{
    inspect_windows_process_callbacks, WindowsCallbackReport, WindowsCallbackWalkLimits,
};
use crate::windows_integrity::{
    check_windows_driver_text_hashes, check_windows_kernel_text_hash, WindowsIntegrityReport,
};
use crate::windows_profile::{WindowsProfile, WindowsProtectionState};
use crate::windows_syscall::{
    inspect_windows_lstar, inspect_windows_ssdt, windows_syscall_path_report, WindowsLstarReport,
    WindowsSsdtReport,
};
use crate::windows_vmi::{WindowsTextRange, WindowsVirtualMemoryReader, WindowsVmiError};
use crate::windows_x64::{inspect_windows_gdt, inspect_windows_idt, WindowsX64DescriptorReport};

pub struct WindowsDetectorInputs<'a> {
    pub profile: &'a WindowsProfile,
    pub memory: &'a dyn WindowsVirtualMemoryReader,
    pub registers: &'a X86_64RegisterSnapshot,
    pub nt_base: u64,
    pub executable_ranges: &'a [WindowsTextRange],
    pub kernel_text_baselines: &'a BTreeMap<String, String>,
    pub driver_text_baselines: &'a BTreeMap<String, String>,
    pub process_callback_slots: usize,
    pub callback_limits: WindowsCallbackWalkLimits,
    pub critical_idt_vectors: &'a [u8],
    pub gdt_selectors: &'a [u16],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsDetectorCheck {
    pub name: &'static str,
    pub ok: bool,
    pub finding_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsDetectorFinding {
    pub check: &'static str,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsProtectionReport {
    pub ok: bool,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsDetectorRun {
    pub ok: bool,
    pub checks: Vec<WindowsDetectorCheck>,
    pub findings: Vec<WindowsDetectorFinding>,
    pub ssdt: WindowsSsdtReport,
    pub lstar: WindowsLstarReport,
    pub syscall_path: SyscallPathReport,
    pub idt: WindowsX64DescriptorReport,
    pub gdt: WindowsX64DescriptorReport,
    pub callbacks: WindowsCallbackReport,
    pub kernel_text: WindowsIntegrityReport,
    pub driver_text: WindowsIntegrityReport,
    pub protection: WindowsProtectionReport,
}

pub fn run_windows_detector_checks(
    inputs: WindowsDetectorInputs<'_>,
) -> Result<WindowsDetectorRun, WindowsVmiError> {
    let ssdt = inspect_windows_ssdt(
        inputs.profile,
        inputs.memory,
        inputs.nt_base,
        inputs.executable_ranges,
    )?;
    let lstar = inspect_windows_lstar(
        inputs.profile,
        inputs.registers,
        inputs.nt_base,
        inputs.executable_ranges,
    )?;
    let syscall_path = windows_syscall_path_report(inputs.profile, &ssdt, &lstar);
    let idt = inspect_windows_idt(
        inputs.memory,
        inputs.registers,
        inputs.executable_ranges,
        inputs.critical_idt_vectors,
    )?;
    let gdt = inspect_windows_gdt(inputs.memory, inputs.registers, inputs.gdt_selectors)?;
    let callbacks = if inputs.process_callback_slots == 0 {
        WindowsCallbackReport {
            ok: true,
            callbacks: Vec::new(),
            findings: Vec::new(),
        }
    } else {
        inspect_windows_process_callbacks(
            inputs.profile,
            inputs.memory,
            inputs.nt_base,
            inputs.executable_ranges,
            inputs.process_callback_slots,
            inputs.callback_limits,
        )?
    };
    let kernel_text = check_windows_kernel_text_hash(
        inputs.memory,
        inputs.executable_ranges,
        inputs.kernel_text_baselines,
    )?;
    let driver_text = check_windows_driver_text_hashes(
        inputs.memory,
        inputs.executable_ranges,
        inputs.driver_text_baselines,
    )?;
    let protection = windows_protection_report(inputs.profile);

    let mut findings = Vec::new();
    add_findings(&mut findings, "lstar", &lstar.findings);
    add_findings(&mut findings, "ssdt", &ssdt.findings);
    add_findings(&mut findings, "idt", &idt.findings);
    add_findings(&mut findings, "gdt", &gdt.findings);
    add_findings(&mut findings, "callbacks", &callbacks.findings);
    add_findings(&mut findings, "kernel_text_hash", &kernel_text.findings);
    add_findings(&mut findings, "driver_text_hash", &driver_text.findings);
    add_findings(&mut findings, "protection_limits", &protection.findings);

    let checks = vec![
        check("lstar", lstar.ok, lstar.findings.len()),
        check("ssdt", ssdt.ok, ssdt.findings.len()),
        check("idt", idt.ok, idt.findings.len()),
        check("gdt", gdt.ok, gdt.findings.len()),
        check("callbacks", callbacks.ok, callbacks.findings.len()),
        check(
            "kernel_text_hash",
            kernel_text.ok,
            kernel_text.findings.len(),
        ),
        check(
            "driver_text_hash",
            driver_text.ok,
            driver_text.findings.len(),
        ),
        check(
            "protection_limits",
            protection.ok,
            protection.findings.len(),
        ),
    ];

    Ok(WindowsDetectorRun {
        ok: findings.is_empty(),
        checks,
        findings,
        ssdt,
        lstar,
        syscall_path,
        idt,
        gdt,
        callbacks,
        kernel_text,
        driver_text,
        protection,
    })
}

pub fn windows_protection_report(profile: &WindowsProfile) -> WindowsProtectionReport {
    let mut findings = Vec::new();
    for limit in profile.limitations() {
        match limit.state {
            WindowsProtectionState::NotPresent => {}
            WindowsProtectionState::Degraded => findings.push(format!(
                "{} support is degraded: {}",
                limit.kind.as_str(),
                limit.detail
            )),
            WindowsProtectionState::Unsupported => findings.push(format!(
                "{} support is unsupported: {}",
                limit.kind.as_str(),
                limit.detail
            )),
        }
    }
    WindowsProtectionReport {
        ok: findings.is_empty(),
        findings,
    }
}

fn check(name: &'static str, ok: bool, finding_count: usize) -> WindowsDetectorCheck {
    WindowsDetectorCheck {
        name,
        ok,
        finding_count,
    }
}

fn add_findings(out: &mut Vec<WindowsDetectorFinding>, check: &'static str, findings: &[String]) {
    out.extend(findings.iter().map(|detail| WindowsDetectorFinding {
        check,
        detail: detail.clone(),
    }));
}
