use std::collections::BTreeMap;

use crate::linux_integrity::sha256_hex;
use crate::windows_vmi::{WindowsTextRange, WindowsVirtualMemoryReader, WindowsVmiError};

const MAX_TEXT_HASH_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsTextHashStatus {
    Match,
    Mismatch,
    UnknownBaseline,
}

impl WindowsTextHashStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Match => "match",
            Self::Mismatch => "mismatch",
            Self::UnknownBaseline => "unknown_baseline",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsTextHashResult {
    pub owner: String,
    pub start: u64,
    pub end: u64,
    pub sha256: String,
    pub expected_sha256: Option<String>,
    pub status: WindowsTextHashStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsIntegrityReport {
    pub ok: bool,
    pub results: Vec<WindowsTextHashResult>,
    pub findings: Vec<String>,
}

pub fn check_windows_kernel_text_hash(
    memory: &dyn WindowsVirtualMemoryReader,
    ranges: &[WindowsTextRange],
    baselines: &BTreeMap<String, String>,
) -> Result<WindowsIntegrityReport, WindowsVmiError> {
    check_windows_text_hashes(
        memory,
        &ranges
            .iter()
            .filter(|range| range.owner.eq_ignore_ascii_case("ntoskrnl.exe"))
            .cloned()
            .collect::<Vec<_>>(),
        baselines,
    )
}

pub fn check_windows_driver_text_hashes(
    memory: &dyn WindowsVirtualMemoryReader,
    ranges: &[WindowsTextRange],
    baselines: &BTreeMap<String, String>,
) -> Result<WindowsIntegrityReport, WindowsVmiError> {
    check_windows_text_hashes(
        memory,
        &ranges
            .iter()
            .filter(|range| !range.owner.eq_ignore_ascii_case("ntoskrnl.exe"))
            .cloned()
            .collect::<Vec<_>>(),
        baselines,
    )
}

pub fn check_windows_text_hashes(
    memory: &dyn WindowsVirtualMemoryReader,
    ranges: &[WindowsTextRange],
    baselines: &BTreeMap<String, String>,
) -> Result<WindowsIntegrityReport, WindowsVmiError> {
    let mut results = Vec::new();
    let mut findings = Vec::new();

    for range in ranges {
        if range.end <= range.start {
            return Err(WindowsVmiError::InconsistentSnapshot {
                detail: format!("text range '{}' is empty or inverted", range.owner),
            });
        }
        let sha256 = hash_virtual_range(memory, range.start, range.end)?;
        let expected = baselines.get(&range.owner).cloned();
        let status = match expected.as_deref() {
            Some(value) if value.eq_ignore_ascii_case(&sha256) => WindowsTextHashStatus::Match,
            Some(_) => WindowsTextHashStatus::Mismatch,
            None => WindowsTextHashStatus::UnknownBaseline,
        };
        if status != WindowsTextHashStatus::Match {
            findings.push(format!(
                "text range '{}' hash status is {}",
                range.owner,
                status.as_str()
            ));
        }
        results.push(WindowsTextHashResult {
            owner: range.owner.clone(),
            start: range.start,
            end: range.end,
            sha256,
            expected_sha256: expected,
            status,
        });
    }

    Ok(WindowsIntegrityReport {
        ok: findings.is_empty(),
        results,
        findings,
    })
}

fn hash_virtual_range(
    memory: &dyn WindowsVirtualMemoryReader,
    start: u64,
    end: u64,
) -> Result<String, WindowsVmiError> {
    let len = end - start;
    if len > MAX_TEXT_HASH_BYTES {
        return Err(WindowsVmiError::Malformed {
            detail: format!("text range length {len} exceeds hashing limit {MAX_TEXT_HASH_BYTES}"),
        });
    }
    let mut bytes =
        Vec::with_capacity(
            usize::try_from(len).map_err(|_| WindowsVmiError::Malformed {
                detail: "text range length does not fit this target".to_string(),
            })?,
        );
    let mut cursor = start;
    let mut buf = [0u8; 4096];
    while cursor < end {
        let remaining = usize::try_from((end - cursor).min(buf.len() as u64)).map_err(|_| {
            WindowsVmiError::Malformed {
                detail: "text range chunk length does not fit this target".to_string(),
            }
        })?;
        memory.read_virtual(cursor, &mut buf[..remaining])?;
        bytes.extend_from_slice(&buf[..remaining]);
        cursor = cursor
            .checked_add(u64::try_from(remaining).expect("chunk length fits u64"))
            .ok_or_else(|| WindowsVmiError::InconsistentSnapshot {
                detail: "text range cursor overflowed".to_string(),
            })?;
    }
    Ok(sha256_hex(&bytes))
}
