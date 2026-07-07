use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::vmi::RegisterReadError;
use crate::vmi_registers::{
    Arm64RegisterSnapshot, DescriptorTableRegister, RegisterSnapshot, X86_64RegisterSnapshot,
};

pub const REGISTER_FIXTURE_VERSION: &str = "aegishv-registers-v1";

pub fn load_register_snapshot_fixture(
    path: impl AsRef<Path>,
) -> Result<RegisterSnapshot, RegisterReadError> {
    let text =
        fs::read_to_string(path).map_err(|err| RegisterReadError::TemporarilyUnavailable {
            resource: "register-fixture",
            detail: format!("cannot read register fixture: {err}"),
        })?;
    parse_register_snapshot_fixture(&text)
}

pub fn parse_register_snapshot_fixture(text: &str) -> Result<RegisterSnapshot, RegisterReadError> {
    let mut lines = logical_lines(text);
    let Some((version_line, version)) = lines.next() else {
        return Err(malformed("missing register fixture version header"));
    };
    if version != REGISTER_FIXTURE_VERSION {
        return Err(malformed(format!(
            "line {version_line}: expected {REGISTER_FIXTURE_VERSION}"
        )));
    }

    let Some((arch_line, arch_entry)) = lines.next() else {
        return Err(malformed("missing register fixture architecture header"));
    };
    let (arch_key, arch) = split_entry(arch_line, arch_entry)?;
    if arch_key != "arch" {
        return Err(malformed(format!(
            "line {arch_line}: expected arch=<architecture>"
        )));
    }

    match arch {
        "x86_64" => parse_x86(lines),
        "arm64" => parse_arm64(lines),
        other => Err(RegisterReadError::UnsupportedArchitecture {
            arch: other.to_string(),
        }),
    }
}

fn parse_x86<'a>(
    lines: impl Iterator<Item = (usize, &'a str)>,
) -> Result<RegisterSnapshot, RegisterReadError> {
    let mut snapshot = X86_64RegisterSnapshot::partial();
    let mut seen = BTreeSet::new();

    for (line, entry) in lines {
        let (key, value) = split_entry(line, entry)?;
        reject_duplicate(&mut seen, line, key)?;
        match key {
            "cr0" => snapshot.cr0 = Some(parse_u64(line, key, value)?),
            "cr2" => snapshot.cr2 = Some(parse_u64(line, key, value)?),
            "cr3" => snapshot.cr3 = Some(parse_u64(line, key, value)?),
            "cr4" => snapshot.cr4 = Some(parse_u64(line, key, value)?),
            "efer" => snapshot.efer = Some(parse_u64(line, key, value)?),
            "idtr.base" => {
                let limit = snapshot
                    .idtr
                    .unwrap_or_else(|| DescriptorTableRegister::new(0, 0))
                    .limit;
                snapshot.idtr = Some(DescriptorTableRegister::new(
                    parse_u64(line, key, value)?,
                    limit,
                ));
            }
            "idtr.limit" => {
                let base = snapshot
                    .idtr
                    .unwrap_or_else(|| DescriptorTableRegister::new(0, 0))
                    .base;
                snapshot.idtr = Some(DescriptorTableRegister::new(
                    base,
                    parse_descriptor_limit(line, key, value)?,
                ));
            }
            "gdtr.base" => {
                let limit = snapshot
                    .gdtr
                    .unwrap_or_else(|| DescriptorTableRegister::new(0, 0))
                    .limit;
                snapshot.gdtr = Some(DescriptorTableRegister::new(
                    parse_u64(line, key, value)?,
                    limit,
                ));
            }
            "gdtr.limit" => {
                let base = snapshot
                    .gdtr
                    .unwrap_or_else(|| DescriptorTableRegister::new(0, 0))
                    .base;
                snapshot.gdtr = Some(DescriptorTableRegister::new(
                    base,
                    parse_descriptor_limit(line, key, value)?,
                ));
            }
            _ => return Err(unknown_key(line, key)),
        }
    }

    snapshot.cr0()?;
    snapshot.cr2()?;
    snapshot.cr3()?;
    snapshot.cr4()?;
    snapshot.efer()?;
    require_key(&seen, "x86_64", "idtr.base")?;
    require_key(&seen, "x86_64", "idtr.limit")?;
    require_key(&seen, "x86_64", "gdtr.base")?;
    require_key(&seen, "x86_64", "gdtr.limit")?;

    Ok(RegisterSnapshot::x86_64(snapshot))
}

fn parse_arm64<'a>(
    lines: impl Iterator<Item = (usize, &'a str)>,
) -> Result<RegisterSnapshot, RegisterReadError> {
    let mut snapshot = Arm64RegisterSnapshot::partial();
    let mut seen = BTreeSet::new();

    for (line, entry) in lines {
        let (key, value) = split_entry(line, entry)?;
        reject_duplicate(&mut seen, line, key)?;
        match key {
            "ttbr0_el1" => snapshot.ttbr0_el1 = Some(parse_u64(line, key, value)?),
            "ttbr1_el1" => snapshot.ttbr1_el1 = Some(parse_u64(line, key, value)?),
            "tcr_el1" => snapshot.tcr_el1 = Some(parse_u64(line, key, value)?),
            "sctlr_el1" => snapshot.sctlr_el1 = Some(parse_u64(line, key, value)?),
            "vbar_el1" => snapshot.vbar_el1 = Some(parse_u64(line, key, value)?),
            _ => return Err(unknown_key(line, key)),
        }
    }

    snapshot.ttbr0_el1()?;
    snapshot.ttbr1_el1()?;
    snapshot.tcr_el1()?;
    snapshot.sctlr_el1()?;
    snapshot.vbar_el1()?;

    Ok(RegisterSnapshot::arm64(snapshot))
}

fn logical_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines().enumerate().filter_map(|(index, line)| {
        let line = line.split_once('#').map_or(line, |(left, _)| left).trim();
        if line.is_empty() {
            None
        } else {
            Some((index + 1, line))
        }
    })
}

fn split_entry(line: usize, entry: &str) -> Result<(&str, &str), RegisterReadError> {
    let Some((key, value)) = entry.split_once('=') else {
        return Err(malformed(format!("line {line}: expected key=value entry")));
    };
    let key = key.trim();
    let value = value.trim();
    if key.is_empty() || value.is_empty() {
        return Err(malformed(format!(
            "line {line}: expected non-empty key and value"
        )));
    }
    Ok((key, value))
}

fn reject_duplicate<'a>(
    seen: &mut BTreeSet<&'a str>,
    line: usize,
    key: &'a str,
) -> Result<(), RegisterReadError> {
    if !seen.insert(key) {
        return Err(malformed(format!(
            "line {line}: duplicate register key '{key}'"
        )));
    }
    Ok(())
}

fn parse_u64(line: usize, field: &str, value: &str) -> Result<u64, RegisterReadError> {
    let parsed = if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16)
    } else {
        value.parse::<u64>()
    };
    parsed.map_err(|_| {
        malformed(format!(
            "line {line}: invalid integer for field '{field}': {value}"
        ))
    })
}

fn parse_descriptor_limit(line: usize, field: &str, value: &str) -> Result<u16, RegisterReadError> {
    let limit = parse_u64(line, field, value)?;
    u16::try_from(limit).map_err(|_| {
        malformed(format!(
            "line {line}: descriptor table limit field '{field}' is out of range: {value}"
        ))
    })
}

fn require_key(
    seen: &BTreeSet<&str>,
    arch: &'static str,
    register: &'static str,
) -> Result<(), RegisterReadError> {
    if seen.contains(register) {
        Ok(())
    } else {
        Err(RegisterReadError::MissingRegister { arch, register })
    }
}

fn unknown_key(line: usize, key: &str) -> RegisterReadError {
    malformed(format!("line {line}: unknown register key '{key}'"))
}

fn malformed(detail: impl Into<String>) -> RegisterReadError {
    RegisterReadError::Malformed {
        detail: detail.into(),
    }
}
