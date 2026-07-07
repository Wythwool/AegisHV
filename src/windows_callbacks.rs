use crate::windows_profile::{WindowsProfile, WindowsStructFieldKey};
use crate::windows_vmi::{
    address_in_windows_text_ranges, read_u64, symbol_address, WindowsTextRange,
    WindowsVirtualMemoryReader, WindowsVmiError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsCallbackWalkLimits {
    pub max_callbacks: usize,
}

impl Default for WindowsCallbackWalkLimits {
    fn default() -> Self {
        Self { max_callbacks: 64 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsCallbackKind {
    ProcessCreate,
}

impl WindowsCallbackKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProcessCreate => "process_create",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsCallbackEntry {
    pub kind: WindowsCallbackKind,
    pub slot: usize,
    pub block_address: u64,
    pub callback: u64,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsCallbackReport {
    pub ok: bool,
    pub callbacks: Vec<WindowsCallbackEntry>,
    pub findings: Vec<String>,
}

pub fn inspect_windows_process_callbacks(
    profile: &WindowsProfile,
    memory: &dyn WindowsVirtualMemoryReader,
    nt_base: u64,
    executable_ranges: &[WindowsTextRange],
    slot_count: usize,
    limits: WindowsCallbackWalkLimits,
) -> Result<WindowsCallbackReport, WindowsVmiError> {
    if executable_ranges.is_empty() {
        return Err(WindowsVmiError::MissingProfileField {
            field: "executable text ranges".to_string(),
        });
    }
    if slot_count == 0 || slot_count > limits.max_callbacks {
        return Err(WindowsVmiError::Malformed {
            detail: format!(
                "process callback slot count {slot_count} must be between 1 and {}",
                limits.max_callbacks
            ),
        });
    }

    let table =
        symbol_address(profile, "PspCreateProcessNotifyRoutine", nt_base).map_err(|_| {
            WindowsVmiError::Unsupported {
                operation: "inspect_process_callbacks",
                detail: "profile is missing symbol:PspCreateProcessNotifyRoutine".to_string(),
            }
        })?;
    let function_offset = optional_offset(profile, "EX_CALLBACK_ROUTINE_BLOCK", "Function");
    let mut callbacks = Vec::new();
    let mut findings = Vec::new();

    for slot in 0..slot_count {
        let entry_address = table
            .checked_add(u64::try_from(slot).expect("callback slot fits u64") * 8)
            .ok_or_else(|| inconsistent("callback table entry address overflowed"))?;
        let encoded = read_u64(memory, entry_address)?;
        let block_address = encoded & !0xf;
        if block_address == 0 {
            continue;
        }

        let callback = function_offset
            .map(|offset| read_u64(memory, checked_add(block_address, offset)?))
            .unwrap_or(Ok(block_address))?;
        let owner = address_in_windows_text_ranges(callback, executable_ranges)
            .map(|range| range.owner.clone());
        if callback != 0 && owner.is_none() {
            findings.push(format!(
                "process callback slot {slot} target 0x{callback:x} is outside executable Windows ranges"
            ));
        }

        callbacks.push(WindowsCallbackEntry {
            kind: WindowsCallbackKind::ProcessCreate,
            slot,
            block_address,
            callback,
            owner,
        });
    }

    Ok(WindowsCallbackReport {
        ok: findings.is_empty(),
        callbacks,
        findings,
    })
}

fn optional_offset(profile: &WindowsProfile, struct_name: &str, field_name: &str) -> Option<u64> {
    let key = WindowsStructFieldKey {
        struct_name: struct_name.to_string(),
        field_name: field_name.to_string(),
    };
    profile.struct_offsets().get(&key).map(|field| field.offset)
}

fn checked_add(base: u64, offset: u64) -> Result<u64, WindowsVmiError> {
    base.checked_add(offset)
        .ok_or_else(|| inconsistent("guest virtual address overflowed"))
}

fn inconsistent(detail: impl Into<String>) -> WindowsVmiError {
    WindowsVmiError::InconsistentSnapshot {
        detail: detail.into(),
    }
}
