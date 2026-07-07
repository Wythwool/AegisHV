use crate::pattern::Pattern;

use super::{TrapError, TrapErrorKind};

#[derive(Debug, Clone)]
pub struct JitTrapRule {
    vm: Option<Pattern>,
    process: Pattern,
    module: Option<Pattern>,
    max_pages: u64,
    max_window_ms: u64,
}

impl JitTrapRule {
    pub fn new(
        vm_regex: Option<&str>,
        process_regex: &str,
        module_regex: Option<&str>,
        max_pages: u64,
        max_window_ms: u64,
    ) -> Result<Self, TrapError> {
        if process_regex.trim().is_empty() {
            return Err(TrapError::new(
                TrapErrorKind::MalformedInput,
                "JIT trap rule requires a process regex",
            ));
        }
        if max_pages == 0 || max_window_ms == 0 {
            return Err(TrapError::new(
                TrapErrorKind::MalformedInput,
                "JIT trap rule requires positive page and window limits",
            ));
        }
        Ok(Self {
            vm: compile_optional(vm_regex, "vm_regex")?,
            process: Pattern::compile(process_regex).map_err(|err| {
                TrapError::new(
                    TrapErrorKind::MalformedInput,
                    format!("invalid JIT process regex '{process_regex}': {err}"),
                )
            })?,
            module: compile_optional(module_regex, "module_regex")?,
            max_pages,
            max_window_ms,
        })
    }

    fn matches(&self, ctx: &JitTrapContext) -> bool {
        self.vm
            .as_ref()
            .map(|pattern| pattern.is_match(&ctx.vm))
            .unwrap_or(true)
            && ctx
                .process
                .as_ref()
                .map(|process| self.process.is_match(process))
                .unwrap_or(false)
            && self
                .module
                .as_ref()
                .map(|pattern| {
                    ctx.module
                        .as_ref()
                        .map(|module| pattern.is_match(module))
                        .unwrap_or(false)
                })
                .unwrap_or(true)
            && ctx.page_count <= self.max_pages
            && ctx.requested_window_ms <= self.max_window_ms
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JitTrapContext {
    pub vm: String,
    pub process: Option<String>,
    pub module: Option<String>,
    pub symbol: Option<String>,
    pub page_count: u64,
    pub requested_window_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JitTrapDecision {
    pub allowed: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct JitTrapPolicy {
    rules: Vec<JitTrapRule>,
}

impl JitTrapPolicy {
    pub fn new(rules: Vec<JitTrapRule>) -> Self {
        Self { rules }
    }

    pub fn evaluate(&self, ctx: &JitTrapContext) -> JitTrapDecision {
        if ctx.process.as_deref().unwrap_or("").trim().is_empty() {
            return JitTrapDecision {
                allowed: false,
                reason: "missing guest process attribution".to_string(),
            };
        }
        if ctx.page_count == 0 || ctx.requested_window_ms == 0 {
            return JitTrapDecision {
                allowed: false,
                reason: "invalid JIT temporary window request".to_string(),
            };
        }
        if self.rules.iter().any(|rule| rule.matches(ctx)) {
            return JitTrapDecision {
                allowed: true,
                reason: "matched JIT trap allow rule".to_string(),
            };
        }
        JitTrapDecision {
            allowed: false,
            reason: "no JIT trap allow rule matched attribution and limits".to_string(),
        }
    }
}

fn compile_optional(value: Option<&str>, field: &str) -> Result<Option<Pattern>, TrapError> {
    match value {
        Some(raw) if !raw.trim().is_empty() => Pattern::compile(raw).map(Some).map_err(|err| {
            TrapError::new(
                TrapErrorKind::MalformedInput,
                format!("invalid JIT {field} '{raw}': {err}"),
            )
        }),
        _ => Ok(None),
    }
}
