use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::vmi::ProfileError;
use crate::windows_profile::{WindowsSymbol, WINDOWS_PROFILE_VERSION};

pub const WINDOWS_SYMBOL_CACHE_VERSION: &str = "aegishv-windows-symbol-cache-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsSymbolCache {
    pub pdb_file: String,
    pub pdb_guid: String,
    pub pdb_age: u32,
    pub source: String,
    pub symbols: BTreeMap<String, WindowsSymbol>,
}

pub fn load_windows_symbol_cache(
    path: impl AsRef<Path>,
) -> Result<WindowsSymbolCache, ProfileError> {
    let text = fs::read_to_string(path).map_err(|err| ProfileError::TemporarilyUnavailable {
        resource: "windows-symbol-cache",
        detail: format!("cannot read Windows symbol cache: {err}"),
    })?;
    parse_windows_symbol_cache(&text)
}

pub fn parse_windows_symbol_cache(text: &str) -> Result<WindowsSymbolCache, ProfileError> {
    let mut lines = logical_lines(text);
    let Some((version_line, version)) = lines.next() else {
        return Err(malformed("missing Windows symbol cache version header"));
    };
    if version != WINDOWS_SYMBOL_CACHE_VERSION {
        return Err(malformed(format!(
            "line {version_line}: expected {WINDOWS_SYMBOL_CACHE_VERSION}"
        )));
    }

    let mut pdb_file = None;
    let mut pdb_guid = None;
    let mut pdb_age = None;
    let mut source = None;
    let mut symbols = BTreeMap::new();

    for (line, entry) in lines {
        let (key, value) = split_entry(line, entry)?;
        match key {
            "pdb_file" => pdb_file = Some(required_text(line, "pdb_file", value)?),
            "pdb_guid" => pdb_guid = Some(required_text(line, "pdb_guid", value)?),
            "pdb_age" => pdb_age = Some(parse_u32(line, "pdb_age", value)?),
            "source" => source = Some(required_text(line, "source", value)?),
            "symbol" => {
                let symbol = parse_symbol(line, value)?;
                if symbols.contains_key(&symbol.name) {
                    return Err(malformed(format!(
                        "line {line}: duplicate symbol '{}'",
                        symbol.name
                    )));
                }
                symbols.insert(symbol.name.clone(), symbol);
            }
            "profile_version" if value == WINDOWS_PROFILE_VERSION => {}
            "profile_version" => {
                return Err(malformed(format!(
                    "line {line}: unsupported Windows profile version '{value}'"
                )))
            }
            _ => {
                return Err(malformed(format!(
                    "line {line}: unknown Windows symbol cache key '{key}'"
                )))
            }
        }
    }

    if symbols.is_empty() {
        return Err(malformed(
            "Windows symbol cache must contain at least one symbol",
        ));
    }
    Ok(WindowsSymbolCache {
        pdb_file: required_field(pdb_file, "pdb_file")?,
        pdb_guid: required_field(pdb_guid, "pdb_guid")?,
        pdb_age: required_field(pdb_age, "pdb_age")?,
        source: required_field(source, "source")?,
        symbols,
    })
}

fn parse_symbol(line: usize, value: &str) -> Result<WindowsSymbol, ProfileError> {
    let parts = value.split(',').map(str::trim).collect::<Vec<_>>();
    if !(2..=3).contains(&parts.len()) {
        return Err(malformed(format!(
            "line {line}: symbol requires name,rva[,size]"
        )));
    }
    let name = required_text(line, "symbol.name", parts[0])?;
    let rva = parse_u64(line, "symbol.rva", parts[1])?;
    let size = parts
        .get(2)
        .map(|value| parse_u64(line, "symbol.size", value))
        .transpose()?;
    Ok(WindowsSymbol { name, rva, size })
}

fn logical_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines().enumerate().filter_map(|(idx, line)| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            None
        } else {
            Some((idx + 1, trimmed))
        }
    })
}

fn split_entry<'a>(line: usize, entry: &'a str) -> Result<(&'a str, &'a str), ProfileError> {
    let Some((key, value)) = entry.split_once('=') else {
        return Err(malformed(format!("line {line}: expected key=value")));
    };
    let key = key.trim();
    let value = value.trim();
    if key.is_empty() || value.is_empty() {
        return Err(malformed(format!(
            "line {line}: key and value must be non-empty"
        )));
    }
    Ok((key, value))
}

fn required_field<T>(value: Option<T>, field: &'static str) -> Result<T, ProfileError> {
    value.ok_or_else(|| {
        malformed(format!(
            "missing required Windows symbol cache field '{field}'"
        ))
    })
}

fn required_text(line: usize, field: &str, value: &str) -> Result<String, ProfileError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(malformed(format!(
            "line {line}: field '{field}' must not be empty"
        )));
    }
    Ok(value.to_string())
}

fn parse_u32(line: usize, field: &str, value: &str) -> Result<u32, ProfileError> {
    let raw = parse_u64(line, field, value)?;
    u32::try_from(raw).map_err(|_| malformed(format!("line {line}: {field} is too large")))
}

fn parse_u64(line: usize, field: &str, value: &str) -> Result<u64, ProfileError> {
    let value = value.trim();
    let parsed = if let Some(hex) = value.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)
    } else {
        value.parse()
    };
    parsed.map_err(|_| {
        malformed(format!(
            "line {line}: invalid integer for {field}: '{value}'"
        ))
    })
}

fn malformed(detail: impl Into<String>) -> ProfileError {
    ProfileError::MalformedProfile {
        detail: detail.into(),
    }
}
