use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::vmi::ProfileError;
use crate::vmi_profiles::{OsKind, OsProfile, ProfileArchitecture, ProfileIdentity};

pub const LINUX_PROFILE_VERSION: &str = "aegishv-linux-profile-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxProfile {
    linux_identity: LinuxProfileIdentity,
    registry_identity: ProfileIdentity,
    profile_name: String,
    kaslr: LinuxKaslrMode,
    kaslr_anchors: Vec<LinuxKaslrAnchor>,
    symbols: BTreeMap<String, LinuxSymbol>,
    struct_offsets: BTreeMap<LinuxStructFieldKey, LinuxStructOffset>,
    syscalls_by_number: BTreeMap<u32, LinuxSyscall>,
}

impl LinuxProfile {
    pub fn linux_identity(&self) -> &LinuxProfileIdentity {
        &self.linux_identity
    }

    pub fn registry_identity(&self) -> &ProfileIdentity {
        &self.registry_identity
    }

    pub fn kaslr(&self) -> LinuxKaslrMode {
        self.kaslr
    }

    pub fn kaslr_anchors(&self) -> &[LinuxKaslrAnchor] {
        &self.kaslr_anchors
    }

    pub fn symbols(&self) -> &BTreeMap<String, LinuxSymbol> {
        &self.symbols
    }

    pub fn struct_offsets(&self) -> &BTreeMap<LinuxStructFieldKey, LinuxStructOffset> {
        &self.struct_offsets
    }

    pub fn syscalls_by_number(&self) -> &BTreeMap<u32, LinuxSyscall> {
        &self.syscalls_by_number
    }
}

impl OsProfile for LinuxProfile {
    fn identity(&self) -> &ProfileIdentity {
        &self.registry_identity
    }

    fn profile_name(&self) -> &str {
        &self.profile_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxProfileIdentity {
    pub kernel_release: String,
    pub kernel_build: String,
    pub variant: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxKaslrMode {
    Fixed,
    SlideKnown { slide: u64 },
    UnknownUnsupported,
}

impl LinuxKaslrMode {
    pub fn slide(self) -> Option<u64> {
        match self {
            Self::SlideKnown { slide } => Some(slide),
            Self::Fixed | Self::UnknownUnsupported => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxSymbol {
    pub name: String,
    pub virtual_address: u64,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxKaslrAnchor {
    pub symbol_name: String,
    pub bytes: Vec<u8>,
    pub max_slide: u64,
    pub step: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxKaslrResolutionSource {
    FixedProfile,
    KnownProfileSlide,
    AnchorScan,
}

impl LinuxKaslrResolutionSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FixedProfile => "fixed_profile",
            Self::KnownProfileSlide => "known_profile_slide",
            Self::AnchorScan => "anchor_scan",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxKaslrResolution {
    pub slide: u64,
    pub source: LinuxKaslrResolutionSource,
    pub anchors_checked: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LinuxStructFieldKey {
    pub struct_name: String,
    pub field_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxStructOffset {
    pub struct_name: String,
    pub field_name: String,
    pub offset: u64,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxSyscall {
    pub number: u32,
    pub name: String,
    pub symbol_name: Option<String>,
}

#[derive(Default)]
struct LinuxProfileBuilder {
    os: Option<OsKind>,
    arch: Option<ProfileArchitecture>,
    kernel_release: Option<String>,
    kernel_build: Option<String>,
    variant: Option<String>,
    kaslr: Option<ParsedKaslrMode>,
    kaslr_slide: Option<u64>,
    kaslr_anchors: Vec<LinuxKaslrAnchor>,
    symbols: BTreeMap<String, LinuxSymbol>,
    struct_offsets: BTreeMap<LinuxStructFieldKey, LinuxStructOffset>,
    syscalls_by_number: BTreeMap<u32, LinuxSyscall>,
    syscall_names: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParsedKaslrMode {
    Fixed,
    SlideKnown,
    UnknownUnsupported,
}

pub fn load_linux_profile(path: impl AsRef<Path>) -> Result<LinuxProfile, ProfileError> {
    let text = fs::read_to_string(path).map_err(|err| ProfileError::TemporarilyUnavailable {
        resource: "linux-profile-fixture",
        detail: format!("cannot read Linux profile fixture: {err}"),
    })?;
    parse_linux_profile(&text)
}

pub fn parse_linux_profile(text: &str) -> Result<LinuxProfile, ProfileError> {
    let mut lines = logical_lines(text);
    let Some((version_line, version)) = lines.next() else {
        return Err(malformed("missing Linux profile version header"));
    };
    if version != LINUX_PROFILE_VERSION {
        return Err(malformed(format!(
            "line {version_line}: expected {LINUX_PROFILE_VERSION}"
        )));
    }

    let mut builder = LinuxProfileBuilder::default();
    let mut singleton_keys = BTreeSet::new();

    for (line, entry) in lines {
        let (key, value) = split_entry(line, entry)?;
        match key {
            "symbol" => parse_symbol(line, value, &mut builder)?,
            "offset" => parse_offset(line, value, &mut builder)?,
            "syscall" => parse_syscall(line, value, &mut builder)?,
            "kaslr_anchor" => parse_kaslr_anchor(line, value, &mut builder)?,
            _ => {
                reject_duplicate_key(&mut singleton_keys, line, key)?;
                parse_top_level(line, key, value, &mut builder)?;
            }
        }
    }

    build_profile(builder)
}

pub fn linux_registry_kernel_or_build(kernel_release: &str, kernel_build: &str) -> String {
    format!("{kernel_release}#{kernel_build}")
}

pub fn resolve_linux_kaslr<F>(
    profile: &LinuxProfile,
    mut read_virtual: F,
) -> Result<LinuxKaslrResolution, ProfileError>
where
    F: FnMut(u64, &mut [u8]) -> Result<(), ProfileError>,
{
    match profile.kaslr {
        LinuxKaslrMode::Fixed => {
            return Ok(LinuxKaslrResolution {
                slide: 0,
                source: LinuxKaslrResolutionSource::FixedProfile,
                anchors_checked: 0,
            })
        }
        LinuxKaslrMode::SlideKnown { slide } => {
            return Ok(LinuxKaslrResolution {
                slide,
                source: LinuxKaslrResolutionSource::KnownProfileSlide,
                anchors_checked: 0,
            })
        }
        LinuxKaslrMode::UnknownUnsupported => {}
    }

    if profile.kaslr_anchors.is_empty() {
        return Err(ProfileError::Unsupported {
            backend: "linux-profile",
            operation: "resolve_kaslr_slide",
        });
    }

    let mut matched_slide = None;
    let mut anchors_checked = 0usize;
    let max_slide = profile
        .kaslr_anchors
        .iter()
        .map(|anchor| anchor.max_slide)
        .max()
        .unwrap_or(0);
    let step = profile
        .kaslr_anchors
        .iter()
        .map(|anchor| anchor.step)
        .min()
        .unwrap_or(0);
    if step == 0 {
        return Err(malformed("KASLR anchor step must be non-zero"));
    }

    let mut slide = 0u64;
    while slide <= max_slide {
        let mut all_match = true;
        for anchor in &profile.kaslr_anchors {
            if slide > anchor.max_slide || slide % anchor.step != 0 {
                all_match = false;
                continue;
            }
            anchors_checked += 1;
            let symbol = profile.symbols.get(&anchor.symbol_name).ok_or_else(|| {
                malformed(format!(
                    "KASLR anchor references missing symbol '{}'",
                    anchor.symbol_name
                ))
            })?;
            let address = symbol.virtual_address.checked_add(slide).ok_or_else(|| {
                malformed(format!(
                    "KASLR candidate slide 0x{slide:x} overflows symbol '{}'",
                    anchor.symbol_name
                ))
            })?;
            let mut buf = vec![0u8; anchor.bytes.len()];
            match read_virtual(address, &mut buf) {
                Ok(()) if buf == anchor.bytes => {}
                Ok(()) => all_match = false,
                Err(err) if err.kind() == crate::vmi::VmiErrorKind::Unmapped => {
                    all_match = false;
                }
                Err(err) if err.kind() == crate::vmi::VmiErrorKind::MissingMemory => {
                    all_match = false;
                }
                Err(err) => return Err(err),
            }
        }
        if all_match {
            if let Some(previous) = matched_slide {
                return Err(ProfileError::InconsistentSnapshot {
                    detail: format!(
                        "KASLR anchor scan matched more than one slide: 0x{previous:x} and 0x{slide:x}"
                    ),
                });
            }
            matched_slide = Some(slide);
        }
        let Some(next) = slide.checked_add(step) else {
            break;
        };
        slide = next;
    }

    let Some(slide) = matched_slide else {
        return Err(ProfileError::MissingProfileIdentity {
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            kernel_or_build: "kaslr-anchor-match".to_string(),
        });
    };

    Ok(LinuxKaslrResolution {
        slide,
        source: LinuxKaslrResolutionSource::AnchorScan,
        anchors_checked,
    })
}

fn parse_top_level(
    line: usize,
    key: &str,
    value: &str,
    builder: &mut LinuxProfileBuilder,
) -> Result<(), ProfileError> {
    match key {
        "os" => builder.os = Some(parse_os(value)?),
        "arch" => builder.arch = Some(parse_architecture(value)?),
        "kernel_release" => {
            builder.kernel_release = Some(normalize_required(line, key, value)?);
        }
        "kernel_build" => builder.kernel_build = Some(normalize_required(line, key, value)?),
        "variant" => builder.variant = Some(normalize_required(line, key, value)?),
        "kaslr" => builder.kaslr = Some(parse_kaslr(line, value)?),
        "kaslr_slide" => builder.kaslr_slide = Some(parse_u64(line, key, value)?),
        _ => {
            return Err(malformed(format!(
                "line {line}: unknown Linux profile key '{key}'"
            )))
        }
    }
    Ok(())
}

fn build_profile(builder: LinuxProfileBuilder) -> Result<LinuxProfile, ProfileError> {
    let os = builder
        .os
        .ok_or_else(|| malformed("missing required Linux profile field 'os'"))?;
    if os != OsKind::Linux {
        return Err(ProfileError::UnsupportedGuest {
            os: os.to_string(),
            arch: builder
                .arch
                .as_ref()
                .map_or_else(|| "*".to_string(), ToString::to_string),
        });
    }

    let arch = builder
        .arch
        .ok_or_else(|| malformed("missing required Linux profile field 'arch'"))?;
    if arch != ProfileArchitecture::X86_64 {
        return Err(ProfileError::UnsupportedArchitecture {
            arch: arch.to_string(),
        });
    }

    let kernel_release = required_string(builder.kernel_release, "kernel_release")?;
    let kernel_build = required_string(builder.kernel_build, "kernel_build")?;
    let linux_identity = LinuxProfileIdentity {
        kernel_release,
        kernel_build,
        variant: builder.variant,
    };

    let kaslr = match builder
        .kaslr
        .ok_or_else(|| malformed("missing required Linux profile field 'kaslr'"))?
    {
        ParsedKaslrMode::Fixed => {
            reject_unexpected_kaslr_slide(builder.kaslr_slide, "fixed")?;
            LinuxKaslrMode::Fixed
        }
        ParsedKaslrMode::SlideKnown => {
            let slide = builder
                .kaslr_slide
                .ok_or_else(|| malformed("kaslr=slide-known requires kaslr_slide"))?;
            LinuxKaslrMode::SlideKnown { slide }
        }
        ParsedKaslrMode::UnknownUnsupported => {
            reject_unexpected_kaslr_slide(builder.kaslr_slide, "unknown-unsupported")?;
            LinuxKaslrMode::UnknownUnsupported
        }
    };

    let registry_identity = ProfileIdentity::new(
        OsKind::Linux,
        ProfileArchitecture::X86_64,
        linux_registry_kernel_or_build(
            &linux_identity.kernel_release,
            &linux_identity.kernel_build,
        ),
        linux_identity.variant.clone(),
    )?;
    let profile_name = format!("synthetic-linux-x86_64-{}", linux_identity.kernel_release);

    Ok(LinuxProfile {
        linux_identity,
        registry_identity,
        profile_name,
        kaslr,
        kaslr_anchors: builder.kaslr_anchors,
        symbols: builder.symbols,
        struct_offsets: builder.struct_offsets,
        syscalls_by_number: builder.syscalls_by_number,
    })
}

fn parse_os(value: &str) -> Result<OsKind, ProfileError> {
    match value {
        "linux" => Ok(OsKind::Linux),
        other => Err(ProfileError::UnsupportedGuest {
            os: other.to_string(),
            arch: "*".to_string(),
        }),
    }
}

fn parse_architecture(value: &str) -> Result<ProfileArchitecture, ProfileError> {
    match value {
        "x86_64" => Ok(ProfileArchitecture::X86_64),
        other => Err(ProfileError::UnsupportedArchitecture {
            arch: other.to_string(),
        }),
    }
}

fn parse_kaslr(line: usize, value: &str) -> Result<ParsedKaslrMode, ProfileError> {
    match value {
        "fixed" | "none" => Ok(ParsedKaslrMode::Fixed),
        "slide-known" => Ok(ParsedKaslrMode::SlideKnown),
        "unknown" | "unsupported" | "unknown-unsupported" => {
            Ok(ParsedKaslrMode::UnknownUnsupported)
        }
        _ => Err(malformed(format!(
            "line {line}: unsupported kaslr mode '{value}'"
        ))),
    }
}

fn parse_symbol(
    line: usize,
    value: &str,
    builder: &mut LinuxProfileBuilder,
) -> Result<(), ProfileError> {
    let parts = split_csv(line, "symbol", value, 2, 3)?;
    let name = normalize_required(line, "symbol.name", parts[0])?;
    if builder.symbols.contains_key(&name) {
        return Err(malformed(format!("line {line}: duplicate symbol '{name}'")));
    }
    let virtual_address = parse_u64(line, "symbol.virtual_address", parts[1])?;
    let size = parts
        .get(2)
        .map(|value| parse_u64(line, "symbol.size", value))
        .transpose()?;

    builder.symbols.insert(
        name.clone(),
        LinuxSymbol {
            name,
            virtual_address,
            size,
        },
    );
    Ok(())
}

fn parse_kaslr_anchor(
    line: usize,
    value: &str,
    builder: &mut LinuxProfileBuilder,
) -> Result<(), ProfileError> {
    let parts = split_csv(line, "kaslr_anchor", value, 4, 4)?;
    let symbol_name = normalize_required(line, "kaslr_anchor.symbol", parts[0])?;
    if builder
        .kaslr_anchors
        .iter()
        .any(|anchor| anchor.symbol_name == symbol_name)
    {
        return Err(malformed(format!(
            "line {line}: duplicate KASLR anchor for symbol '{symbol_name}'"
        )));
    }
    let bytes = parse_hex_bytes(line, "kaslr_anchor.bytes", parts[1])?;
    let max_slide = parse_u64(line, "kaslr_anchor.max_slide", parts[2])?;
    let step = parse_u64(line, "kaslr_anchor.step", parts[3])?;
    if step == 0 || !step.is_power_of_two() {
        return Err(malformed(format!(
            "line {line}: kaslr_anchor.step must be a non-zero power of two"
        )));
    }
    if max_slide % step != 0 {
        return Err(malformed(format!(
            "line {line}: kaslr_anchor.max_slide must be aligned to step"
        )));
    }
    builder.kaslr_anchors.push(LinuxKaslrAnchor {
        symbol_name,
        bytes,
        max_slide,
        step,
    });
    Ok(())
}

fn parse_offset(
    line: usize,
    value: &str,
    builder: &mut LinuxProfileBuilder,
) -> Result<(), ProfileError> {
    let parts = split_csv(line, "offset", value, 3, 4)?;
    let struct_name = normalize_required(line, "offset.struct", parts[0])?;
    let field_name = normalize_required(line, "offset.field", parts[1])?;
    let key = LinuxStructFieldKey {
        struct_name: struct_name.clone(),
        field_name: field_name.clone(),
    };
    if builder.struct_offsets.contains_key(&key) {
        return Err(malformed(format!(
            "line {line}: duplicate struct offset '{}.{}'",
            key.struct_name, key.field_name
        )));
    }
    let offset = parse_u64(line, "offset.byte_offset", parts[2])?;
    let size = parts
        .get(3)
        .map(|value| parse_u64(line, "offset.size", value))
        .transpose()?;

    builder.struct_offsets.insert(
        key,
        LinuxStructOffset {
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
    builder: &mut LinuxProfileBuilder,
) -> Result<(), ProfileError> {
    let parts = split_csv(line, "syscall", value, 2, 3)?;
    let number = parse_u32(line, "syscall.number", parts[0])?;
    if builder.syscalls_by_number.contains_key(&number) {
        return Err(malformed(format!(
            "line {line}: duplicate syscall number {number}"
        )));
    }
    let name = normalize_required(line, "syscall.name", parts[1])?;
    if !builder.syscall_names.insert(name.clone()) {
        return Err(malformed(format!(
            "line {line}: duplicate syscall name '{name}'"
        )));
    }
    let symbol_name = parts
        .get(2)
        .map(|value| normalize_required(line, "syscall.symbol", value))
        .transpose()?;

    builder.syscalls_by_number.insert(
        number,
        LinuxSyscall {
            number,
            name,
            symbol_name,
        },
    );
    Ok(())
}

fn reject_unexpected_kaslr_slide(slide: Option<u64>, mode: &str) -> Result<(), ProfileError> {
    if slide.is_some() {
        Err(malformed(format!("kaslr={mode} must not set kaslr_slide")))
    } else {
        Ok(())
    }
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

fn split_entry(line: usize, entry: &str) -> Result<(&str, &str), ProfileError> {
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

fn split_csv<'a>(
    line: usize,
    field: &str,
    value: &'a str,
    min: usize,
    max: usize,
) -> Result<Vec<&'a str>, ProfileError> {
    let parts: Vec<_> = value.split(',').map(str::trim).collect();
    if parts.len() < min || parts.len() > max || parts.iter().any(|part| part.is_empty()) {
        return Err(malformed(format!(
            "line {line}: field '{field}' expects {min} to {max} comma-separated values"
        )));
    }
    Ok(parts)
}

fn reject_duplicate_key<'a>(
    seen: &mut BTreeSet<&'a str>,
    line: usize,
    key: &'a str,
) -> Result<(), ProfileError> {
    if !seen.insert(key) {
        return Err(malformed(format!(
            "line {line}: duplicate Linux profile key '{key}'"
        )));
    }
    Ok(())
}

fn required_string(value: Option<String>, field: &str) -> Result<String, ProfileError> {
    value.ok_or_else(|| malformed(format!("missing required Linux profile field '{field}'")))
}

fn normalize_required(line: usize, field: &str, value: &str) -> Result<String, ProfileError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(malformed(format!(
            "line {line}: field '{field}' must not be empty"
        )));
    }
    Ok(value.to_string())
}

fn parse_u64(line: usize, field: &str, value: &str) -> Result<u64, ProfileError> {
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

fn parse_u32(line: usize, field: &str, value: &str) -> Result<u32, ProfileError> {
    let parsed = parse_u64(line, field, value)?;
    u32::try_from(parsed).map_err(|_| {
        malformed(format!(
            "line {line}: integer for field '{field}' is out of range: {value}"
        ))
    })
}

fn parse_hex_bytes(line: usize, field: &str, value: &str) -> Result<Vec<u8>, ProfileError> {
    let hex = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    if hex.is_empty() || hex.len() % 2 != 0 {
        return Err(malformed(format!(
            "line {line}: field '{field}' must contain full hex bytes"
        )));
    }
    if hex.len() > 512 {
        return Err(malformed(format!(
            "line {line}: field '{field}' is too large for bounded KASLR scan"
        )));
    }
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair)
                .map_err(|_| malformed(format!("line {line}: field '{field}' must be ASCII")))?;
            u8::from_str_radix(text, 16).map_err(|_| {
                malformed(format!(
                    "line {line}: invalid hex byte '{text}' in field '{field}'"
                ))
            })
        })
        .collect()
}

fn malformed(detail: impl Into<String>) -> ProfileError {
    ProfileError::MalformedProfile {
        detail: detail.into(),
    }
}
