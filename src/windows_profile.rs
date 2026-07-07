use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::vmi::ProfileError;
use crate::vmi_profiles::{OsKind, OsProfile, ProfileArchitecture, ProfileIdentity};

pub const WINDOWS_PROFILE_VERSION: &str = "aegishv-windows-profile-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsProfile {
    windows_identity: WindowsProfileIdentity,
    registry_identity: ProfileIdentity,
    profile_name: String,
    symbols: BTreeMap<String, WindowsSymbol>,
    struct_offsets: BTreeMap<WindowsStructFieldKey, WindowsStructOffset>,
    syscalls_by_number: BTreeMap<u32, WindowsSyscall>,
    limitations: Vec<WindowsProtectionLimit>,
}

impl WindowsProfile {
    pub fn windows_identity(&self) -> &WindowsProfileIdentity {
        &self.windows_identity
    }

    pub fn registry_identity(&self) -> &ProfileIdentity {
        &self.registry_identity
    }

    pub fn symbols(&self) -> &BTreeMap<String, WindowsSymbol> {
        &self.symbols
    }

    pub fn struct_offsets(&self) -> &BTreeMap<WindowsStructFieldKey, WindowsStructOffset> {
        &self.struct_offsets
    }

    pub fn syscalls_by_number(&self) -> &BTreeMap<u32, WindowsSyscall> {
        &self.syscalls_by_number
    }

    pub fn limitations(&self) -> &[WindowsProtectionLimit] {
        &self.limitations
    }
}

impl OsProfile for WindowsProfile {
    fn identity(&self) -> &ProfileIdentity {
        &self.registry_identity
    }

    fn profile_name(&self) -> &str {
        &self.profile_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsProfileIdentity {
    pub build: String,
    pub pdb_file: String,
    pub pdb_guid: String,
    pub pdb_age: u32,
    pub variant: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsSymbol {
    pub name: String,
    pub rva: u64,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WindowsStructFieldKey {
    pub struct_name: String,
    pub field_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsStructOffset {
    pub struct_name: String,
    pub field_name: String,
    pub offset: u64,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsSyscall {
    pub number: u32,
    pub name: String,
    pub symbol_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsProtectionKind {
    Vbs,
    Hvci,
    ConfidentialGuest,
}

impl WindowsProtectionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vbs => "vbs",
            Self::Hvci => "hvci",
            Self::ConfidentialGuest => "confidential_guest",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsProtectionState {
    NotPresent,
    Degraded,
    Unsupported,
}

impl WindowsProtectionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotPresent => "not_present",
            Self::Degraded => "degraded",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsProtectionLimit {
    pub kind: WindowsProtectionKind,
    pub state: WindowsProtectionState,
    pub detail: String,
}

#[derive(Default)]
struct WindowsProfileBuilder {
    os: Option<OsKind>,
    arch: Option<ProfileArchitecture>,
    build: Option<String>,
    pdb_file: Option<String>,
    pdb_guid: Option<String>,
    pdb_age: Option<u32>,
    variant: Option<String>,
    symbols: BTreeMap<String, WindowsSymbol>,
    struct_offsets: BTreeMap<WindowsStructFieldKey, WindowsStructOffset>,
    syscalls_by_number: BTreeMap<u32, WindowsSyscall>,
    syscall_names: BTreeSet<String>,
    limitations: Vec<WindowsProtectionLimit>,
}

pub fn load_windows_profile(path: impl AsRef<Path>) -> Result<WindowsProfile, ProfileError> {
    let text = fs::read_to_string(path).map_err(|err| ProfileError::TemporarilyUnavailable {
        resource: "windows-profile-fixture",
        detail: format!("cannot read Windows profile fixture: {err}"),
    })?;
    parse_windows_profile(&text)
}

pub fn parse_windows_profile(text: &str) -> Result<WindowsProfile, ProfileError> {
    let mut lines = logical_lines(text);
    let Some((version_line, version)) = lines.next() else {
        return Err(malformed("missing Windows profile version header"));
    };
    if version != WINDOWS_PROFILE_VERSION {
        return Err(malformed(format!(
            "line {version_line}: expected {WINDOWS_PROFILE_VERSION}"
        )));
    }

    let mut builder = WindowsProfileBuilder::default();
    let mut singleton_keys = BTreeSet::new();
    for (line, entry) in lines {
        let (key, value) = split_entry(line, entry)?;
        match key {
            "symbol" => parse_symbol(line, value, &mut builder)?,
            "offset" => parse_offset(line, value, &mut builder)?,
            "syscall" => parse_syscall(line, value, &mut builder)?,
            "limit" => parse_limit(line, value, &mut builder)?,
            _ => {
                reject_duplicate_key(&mut singleton_keys, line, key)?;
                parse_top_level(line, key, value, &mut builder)?;
            }
        }
    }

    build_profile(builder)
}

pub fn windows_registry_build(build: &str, pdb_guid: &str, pdb_age: u32) -> String {
    format!("{build}#{pdb_guid}#{pdb_age}")
}

fn build_profile(builder: WindowsProfileBuilder) -> Result<WindowsProfile, ProfileError> {
    let os = required_field(builder.os, "os")?;
    let arch = required_field(builder.arch, "arch")?;
    if os != OsKind::Windows {
        return Err(ProfileError::UnsupportedGuest {
            os: os.to_string(),
            arch: arch.to_string(),
        });
    }
    if arch != ProfileArchitecture::X86_64 {
        return Err(ProfileError::UnsupportedArchitecture {
            arch: arch.to_string(),
        });
    }
    let build = required_field(builder.build, "build")?;
    let pdb_file = required_field(builder.pdb_file, "pdb_file")?;
    let pdb_guid = required_field(builder.pdb_guid, "pdb_guid")?;
    let pdb_age = required_field(builder.pdb_age, "pdb_age")?;
    if builder.symbols.is_empty() {
        return Err(malformed(
            "Windows profile must contain at least one symbol",
        ));
    }
    let registry_identity = ProfileIdentity::new(
        OsKind::Windows,
        ProfileArchitecture::X86_64,
        windows_registry_build(&build, &pdb_guid, pdb_age),
        builder.variant.clone(),
    )?;
    let profile_name = builder
        .variant
        .as_ref()
        .map(|variant| format!("windows-{build}-{variant}"))
        .unwrap_or_else(|| format!("windows-{build}"));
    Ok(WindowsProfile {
        windows_identity: WindowsProfileIdentity {
            build,
            pdb_file,
            pdb_guid,
            pdb_age,
            variant: builder.variant,
        },
        registry_identity,
        profile_name,
        symbols: builder.symbols,
        struct_offsets: builder.struct_offsets,
        syscalls_by_number: builder.syscalls_by_number,
        limitations: builder.limitations,
    })
}

fn parse_top_level(
    line: usize,
    key: &str,
    value: &str,
    builder: &mut WindowsProfileBuilder,
) -> Result<(), ProfileError> {
    match key {
        "os" => {
            builder.os = Some(match value {
                "windows" => OsKind::Windows,
                other => OsKind::Other(other.to_string()),
            });
        }
        "arch" => {
            builder.arch = Some(match value {
                "x86_64" => ProfileArchitecture::X86_64,
                other => ProfileArchitecture::Other(other.to_string()),
            });
        }
        "build" => builder.build = Some(required_text(line, "build", value)?),
        "variant" => builder.variant = Some(required_text(line, "variant", value)?),
        "pdb_file" => builder.pdb_file = Some(required_text(line, "pdb_file", value)?),
        "pdb_guid" => builder.pdb_guid = Some(required_text(line, "pdb_guid", value)?),
        "pdb_age" => builder.pdb_age = Some(parse_u32(line, "pdb_age", value)?),
        _ => {
            return Err(malformed(format!(
                "line {line}: unknown Windows profile key '{key}'"
            )))
        }
    }
    Ok(())
}

fn parse_symbol(
    line: usize,
    value: &str,
    builder: &mut WindowsProfileBuilder,
) -> Result<(), ProfileError> {
    let parts = split_csv(value);
    if !(2..=3).contains(&parts.len()) {
        return Err(malformed(format!(
            "line {line}: symbol requires name,rva[,size]"
        )));
    }
    let name = required_text(line, "symbol.name", parts[0])?;
    if builder.symbols.contains_key(&name) {
        return Err(malformed(format!("line {line}: duplicate symbol '{name}'")));
    }
    let rva = parse_u64(line, "symbol.rva", parts[1])?;
    let size = parts
        .get(2)
        .map(|value| parse_u64(line, "symbol.size", value))
        .transpose()?;
    builder
        .symbols
        .insert(name.clone(), WindowsSymbol { name, rva, size });
    Ok(())
}

fn parse_offset(
    line: usize,
    value: &str,
    builder: &mut WindowsProfileBuilder,
) -> Result<(), ProfileError> {
    let parts = split_csv(value);
    if !(3..=4).contains(&parts.len()) {
        return Err(malformed(format!(
            "line {line}: offset requires struct,field,offset[,size]"
        )));
    }
    let struct_name = required_text(line, "offset.struct", parts[0])?;
    let field_name = required_text(line, "offset.field", parts[1])?;
    let key = WindowsStructFieldKey {
        struct_name: struct_name.clone(),
        field_name: field_name.clone(),
    };
    if builder.struct_offsets.contains_key(&key) {
        return Err(malformed(format!(
            "line {line}: duplicate offset {struct_name}.{field_name}"
        )));
    }
    let offset = parse_u64(line, "offset.offset", parts[2])?;
    let size = parts
        .get(3)
        .map(|value| parse_u64(line, "offset.size", value))
        .transpose()?;
    builder.struct_offsets.insert(
        key,
        WindowsStructOffset {
            struct_name,
            field_name,
            offset,
            size,
        },
    );
    Ok(())
}

fn parse_syscall(
    line: usize,
    value: &str,
    builder: &mut WindowsProfileBuilder,
) -> Result<(), ProfileError> {
    let parts = split_csv(value);
    if !(2..=3).contains(&parts.len()) {
        return Err(malformed(format!(
            "line {line}: syscall requires number,name[,symbol]"
        )));
    }
    let number = parse_u32(line, "syscall.number", parts[0])?;
    let name = required_text(line, "syscall.name", parts[1])?;
    if builder.syscalls_by_number.contains_key(&number) {
        return Err(malformed(format!(
            "line {line}: duplicate syscall number {number}"
        )));
    }
    if !builder.syscall_names.insert(name.clone()) {
        return Err(malformed(format!(
            "line {line}: duplicate syscall name '{name}'"
        )));
    }
    let symbol_name = parts
        .get(2)
        .map(|value| required_text(line, "syscall.symbol", value))
        .transpose()?;
    builder.syscalls_by_number.insert(
        number,
        WindowsSyscall {
            number,
            name,
            symbol_name,
        },
    );
    Ok(())
}

fn parse_limit(
    line: usize,
    value: &str,
    builder: &mut WindowsProfileBuilder,
) -> Result<(), ProfileError> {
    let parts = split_csv(value);
    if parts.len() != 3 {
        return Err(malformed(format!(
            "line {line}: limit requires kind,state,detail"
        )));
    }
    let kind = match parts[0] {
        "vbs" => WindowsProtectionKind::Vbs,
        "hvci" => WindowsProtectionKind::Hvci,
        "confidential_guest" => WindowsProtectionKind::ConfidentialGuest,
        other => {
            return Err(malformed(format!(
                "line {line}: unsupported Windows protection kind '{other}'"
            )))
        }
    };
    let state = match parts[1] {
        "not_present" => WindowsProtectionState::NotPresent,
        "degraded" => WindowsProtectionState::Degraded,
        "unsupported" => WindowsProtectionState::Unsupported,
        other => {
            return Err(malformed(format!(
                "line {line}: unsupported Windows protection state '{other}'"
            )))
        }
    };
    builder.limitations.push(WindowsProtectionLimit {
        kind,
        state,
        detail: required_text(line, "limit.detail", parts[2])?,
    });
    Ok(())
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

fn split_csv(value: &str) -> Vec<&str> {
    value.split(',').map(str::trim).collect()
}

fn reject_duplicate_key(
    seen: &mut BTreeSet<String>,
    line: usize,
    key: &str,
) -> Result<(), ProfileError> {
    if !seen.insert(key.to_string()) {
        return Err(malformed(format!("line {line}: duplicate key '{key}'")));
    }
    Ok(())
}

fn required_field<T>(value: Option<T>, field: &'static str) -> Result<T, ProfileError> {
    value.ok_or_else(|| malformed(format!("missing required Windows profile field '{field}'")))
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
