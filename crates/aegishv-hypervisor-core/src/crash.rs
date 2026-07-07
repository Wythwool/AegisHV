use crate::error::{CoreError, CoreErrorKind};
use crate::ids::PhysicalCpuId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrashCode {
    Panic,
    MachineCheck,
    TripleFault,
    Assertion,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedText<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> FixedText<N> {
    pub fn new(text: &str) -> Result<Self, CoreError> {
        if text.is_empty() {
            return Err(CoreError::new(
                CoreErrorKind::InvalidArgument,
                "fixed text must not be empty",
            ));
        }
        if text.len() > N {
            return Err(CoreError::new(
                CoreErrorKind::CapacityExceeded,
                "fixed text does not fit in the crash record field",
            ));
        }

        let mut bytes = [0; N];
        bytes[..text.len()].copy_from_slice(text.as_bytes());
        Ok(Self {
            bytes,
            len: text.len(),
        })
    }

    pub fn as_str(&self) -> &str {
        match core::str::from_utf8(&self.bytes[..self.len]) {
            Ok(text) => text,
            Err(_) => "",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrashReason {
    pub code: CrashCode,
    pub detail: FixedText<96>,
}

impl CrashReason {
    pub fn new(code: CrashCode, detail: &str) -> Result<Self, CoreError> {
        Ok(Self {
            code,
            detail: FixedText::new(detail)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub git_rev: &'static str,
}

impl BuildInfo {
    pub const fn new(
        name: &'static str,
        version: &'static str,
        git_rev: &'static str,
    ) -> Result<Self, CoreError> {
        if name.is_empty() || version.is_empty() || git_rev.is_empty() {
            Err(CoreError::new(
                CoreErrorKind::InvalidArgument,
                "build info requires name, version, and git revision",
            ))
        } else {
            Ok(Self {
                name,
                version,
                git_rev,
            })
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrashRecord {
    pub cpu: PhysicalCpuId,
    pub reason: CrashReason,
    pub instruction_pointer: Option<u64>,
    pub build: BuildInfo,
}

impl CrashRecord {
    pub fn new(
        cpu: PhysicalCpuId,
        reason: CrashReason,
        instruction_pointer: Option<u64>,
        build: BuildInfo,
    ) -> Self {
        Self {
            cpu,
            reason,
            instruction_pointer,
            build,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_record_keeps_cpu_reason_ip_and_build_info_without_heap() {
        let reason = CrashReason::new(CrashCode::Panic, "panic handler entered").unwrap();
        let build = BuildInfo::new("aegishv-type1", "0.1.0", "local-test").unwrap();
        let record = CrashRecord::new(
            PhysicalCpuId::new(2).unwrap(),
            reason,
            Some(0xffff_8000_0000_1000),
            build,
        );

        assert_eq!(record.cpu.get(), 2);
        assert_eq!(record.reason.detail.as_str(), "panic handler entered");
        assert_eq!(record.instruction_pointer, Some(0xffff_8000_0000_1000));
        assert_eq!(record.build.git_rev, "local-test");
    }

    #[test]
    fn crash_reason_rejects_empty_or_oversized_detail() {
        assert_eq!(
            CrashReason::new(CrashCode::Assertion, "").unwrap_err().kind,
            CoreErrorKind::InvalidArgument
        );
        assert_eq!(
            CrashReason::new(CrashCode::Assertion, "x".repeat(97).as_str())
                .unwrap_err()
                .kind,
            CoreErrorKind::CapacityExceeded
        );
    }
}
