use crate::detectors::memory::{MemoryMapping, MemoryRegionKind};
use crate::detectors::DetectorError;
use crate::pattern::Pattern;

#[derive(Debug, Clone)]
pub struct JitAllowRule {
    process: Option<Pattern>,
    module: Option<Pattern>,
    min_size: u64,
    max_size: u64,
}

impl JitAllowRule {
    pub fn new(
        process_pattern: Option<&str>,
        module_pattern: Option<&str>,
        min_size: u64,
        max_size: u64,
    ) -> Result<Self, DetectorError> {
        if max_size == 0 || min_size > max_size {
            return Err(DetectorError::MalformedInput {
                detail: format!("JIT allow rule size range {min_size}..{max_size} is invalid"),
            });
        }
        let process = process_pattern
            .filter(|value| !value.trim().is_empty())
            .map(Pattern::compile)
            .transpose()
            .map_err(|err| DetectorError::MalformedInput {
                detail: format!("invalid JIT process pattern: {err}"),
            })?;
        let module = module_pattern
            .filter(|value| !value.trim().is_empty())
            .map(Pattern::compile)
            .transpose()
            .map_err(|err| DetectorError::MalformedInput {
                detail: format!("invalid JIT module pattern: {err}"),
            })?;
        Ok(Self {
            process,
            module,
            min_size,
            max_size,
        })
    }

    fn allows(&self, mapping: &MemoryMapping) -> bool {
        if mapping.kind != MemoryRegionKind::Jit && mapping.kind != MemoryRegionKind::Anonymous {
            return false;
        }
        let size = mapping.end.saturating_sub(mapping.start);
        if size < self.min_size || size > self.max_size {
            return false;
        }
        let process_ok = self
            .process
            .as_ref()
            .map(|pattern| {
                mapping
                    .process
                    .as_deref()
                    .map(|process| pattern.is_match(process))
                    .unwrap_or(false)
            })
            .unwrap_or(true);
        let module_ok = self
            .module
            .as_ref()
            .map(|pattern| {
                mapping
                    .module
                    .as_deref()
                    .map(|module| pattern.is_match(module))
                    .unwrap_or(false)
            })
            .unwrap_or(true);
        process_ok && module_ok
    }
}

#[derive(Debug, Clone, Default)]
pub struct JitAllowlist {
    rules: Vec<JitAllowRule>,
}

impl JitAllowlist {
    pub fn new(rules: Vec<JitAllowRule>) -> Self {
        Self { rules }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn allows(&self, mapping: &MemoryMapping) -> bool {
        self.rules.iter().any(|rule| rule.allows(mapping))
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}
