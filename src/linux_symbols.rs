use std::collections::BTreeMap;

use crate::vmi::ProfileError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxKernelSymbol {
    pub address: u64,
    pub kind: char,
    pub name: String,
    pub module: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinuxSymbolTable {
    by_name: BTreeMap<String, Vec<LinuxKernelSymbol>>,
    ordered: Vec<LinuxKernelSymbol>,
}

impl LinuxSymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, symbol: LinuxKernelSymbol) {
        self.by_name
            .entry(symbol.name.clone())
            .or_default()
            .push(symbol.clone());
        self.ordered.push(symbol);
        self.ordered
            .sort_by_key(|symbol| (symbol.address, symbol.name.clone()));
    }

    pub fn symbols(&self) -> &[LinuxKernelSymbol] {
        &self.ordered
    }

    pub fn by_name(&self, name: &str) -> &[LinuxKernelSymbol] {
        self.by_name
            .get(name)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn unique_by_name(&self, name: &str) -> Result<&LinuxKernelSymbol, ProfileError> {
        let matches = self.by_name(name);
        match matches {
            [] => Err(ProfileError::MissingProfileIdentity {
                os: "linux".to_string(),
                arch: "x86_64".to_string(),
                kernel_or_build: format!("symbol:{name}"),
            }),
            [symbol] => Ok(symbol),
            _ => Err(ProfileError::InconsistentSnapshot {
                detail: format!("symbol '{name}' is ambiguous in guest symbol table"),
            }),
        }
    }

    pub fn symbol_containing(&self, address: u64) -> Option<&LinuxKernelSymbol> {
        self.ordered.iter().rev().find(|symbol| {
            symbol.address <= address
                && symbol
                    .module
                    .as_deref()
                    .map(|module| !module.is_empty())
                    .unwrap_or(true)
        })
    }
}

pub fn parse_kallsyms_text(text: &str) -> Result<LinuxSymbolTable, ProfileError> {
    parse_symbol_map_text(text, LinuxSymbolFormat::Kallsyms)
}

pub fn parse_system_map_text(text: &str) -> Result<LinuxSymbolTable, ProfileError> {
    parse_symbol_map_text(text, LinuxSymbolFormat::SystemMap)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinuxSymbolFormat {
    Kallsyms,
    SystemMap,
}

fn parse_symbol_map_text(
    text: &str,
    format: LinuxSymbolFormat,
) -> Result<LinuxSymbolTable, ProfileError> {
    let mut table = LinuxSymbolTable::new();
    for (index, raw_line) in text.lines().enumerate() {
        let line_no = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        table.insert(parse_symbol_line(line_no, line, format)?);
    }
    if table.symbols().is_empty() {
        return Err(malformed("symbol map does not contain any symbols"));
    }
    Ok(table)
}

fn parse_symbol_line(
    line_no: usize,
    line: &str,
    format: LinuxSymbolFormat,
) -> Result<LinuxKernelSymbol, ProfileError> {
    let mut parts = line.split_whitespace();
    let address = parse_hex_u64(line_no, required(line_no, parts.next(), "address")?)?;
    let kind = parse_kind(line_no, required(line_no, parts.next(), "type")?)?;
    let name = required(line_no, parts.next(), "name")?.to_string();
    let module = parts.next().map(parse_module_name).transpose()?;
    if parts.next().is_some() {
        return Err(malformed(format!(
            "line {line_no}: symbol line has too many fields"
        )));
    }
    if !name_is_reasonable(&name) {
        return Err(malformed(format!(
            "line {line_no}: symbol name '{name}' is not valid"
        )));
    }
    if matches!(format, LinuxSymbolFormat::SystemMap) && module.is_some() {
        return Err(malformed(format!(
            "line {line_no}: System.map entries must not include a module suffix"
        )));
    }
    Ok(LinuxKernelSymbol {
        address,
        kind,
        name,
        module,
    })
}

fn required<'a>(
    line_no: usize,
    value: Option<&'a str>,
    field: &str,
) -> Result<&'a str, ProfileError> {
    value.ok_or_else(|| malformed(format!("line {line_no}: missing symbol {field}")))
}

fn parse_hex_u64(line_no: usize, value: &str) -> Result<u64, ProfileError> {
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    if value.is_empty() || value.len() > 16 {
        return Err(malformed(format!(
            "line {line_no}: invalid symbol address '{value}'"
        )));
    }
    u64::from_str_radix(value, 16)
        .map_err(|_| malformed(format!("line {line_no}: invalid symbol address '{value}'")))
}

fn parse_kind(line_no: usize, value: &str) -> Result<char, ProfileError> {
    let mut chars = value.chars();
    let Some(kind) = chars.next() else {
        return Err(malformed(format!("line {line_no}: empty symbol type")));
    };
    if chars.next().is_some() || !kind.is_ascii_alphabetic() {
        return Err(malformed(format!(
            "line {line_no}: invalid symbol type '{value}'"
        )));
    }
    Ok(kind)
}

fn parse_module_name(value: &str) -> Result<String, ProfileError> {
    let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) else {
        return Err(malformed(format!(
            "module suffix '{value}' must use [module] form"
        )));
    };
    if inner.is_empty() || !name_is_reasonable(inner) {
        return Err(malformed(format!("module name '{inner}' is not valid")));
    }
    Ok(inner.to_string())
}

fn name_is_reasonable(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

fn malformed(detail: impl Into<String>) -> ProfileError {
    ProfileError::MalformedProfile {
        detail: detail.into(),
    }
}
