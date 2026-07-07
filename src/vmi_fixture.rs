use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::vmi::{
    GuestPhysical, GuestVirtual, MemoryReadError, ProfileError, RegisterReadError,
    TranslationError, TranslationResult, VmiErrorKind,
};
use crate::vmi_cache::{Arm64CacheGranule, TranslationMode};
use crate::vmi_profiles::{OsKind, OsProfileRegistry, ProfileArchitecture, ProfileIdentity};
use crate::vmi_register_fixtures::load_register_snapshot_fixture;
use crate::vmi_registers::RegisterSnapshot;
use crate::vmi_snapshot::OfflineGuestMemorySnapshotReader;

pub const VMI_FIXTURE_VERSION: &str = "aegishv-vmi-fixture-v1";

#[derive(Debug)]
pub enum VmiFixtureError {
    Malformed {
        detail: String,
    },
    DuplicateFixtureId {
        id: String,
    },
    DuplicateExpectedTranslation {
        name: String,
    },
    PermissionDenied {
        operation: &'static str,
        detail: String,
    },
    TemporarilyUnavailable {
        resource: &'static str,
        detail: String,
    },
    Memory(MemoryReadError),
    Registers(RegisterReadError),
    Profile(ProfileError),
    Translation(TranslationError),
}

impl VmiFixtureError {
    pub fn kind(&self) -> VmiErrorKind {
        match self {
            Self::Malformed { .. }
            | Self::DuplicateFixtureId { .. }
            | Self::DuplicateExpectedTranslation { .. } => VmiErrorKind::Malformed,
            Self::PermissionDenied { .. } => VmiErrorKind::PermissionDenied,
            Self::TemporarilyUnavailable { .. } => VmiErrorKind::TemporarilyUnavailable,
            Self::Memory(err) => err.kind(),
            Self::Registers(err) => err.kind(),
            Self::Profile(err) => err.kind(),
            Self::Translation(err) => err.kind(),
        }
    }
}

impl fmt::Display for VmiFixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { detail } => write!(f, "VMI fixture manifest is malformed: {detail}"),
            Self::DuplicateFixtureId { id } => {
                write!(f, "duplicate VMI fixture id '{id}'")
            }
            Self::DuplicateExpectedTranslation { name } => {
                write!(f, "duplicate expected translation name '{name}'")
            }
            Self::PermissionDenied { operation, detail } => {
                write!(
                    f,
                    "VMI fixture operation '{operation}' is not permitted: {detail}"
                )
            }
            Self::TemporarilyUnavailable { resource, detail } => {
                write!(
                    f,
                    "VMI fixture resource '{resource}' is unavailable: {detail}"
                )
            }
            Self::Memory(err) => write!(f, "{err}"),
            Self::Registers(err) => write!(f, "{err}"),
            Self::Profile(err) => write!(f, "{err}"),
            Self::Translation(err) => write!(f, "{err}"),
        }
    }
}

impl Error for VmiFixtureError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(err) => Some(err),
            Self::Registers(err) => Some(err),
            Self::Profile(err) => Some(err),
            Self::Translation(err) => Some(err),
            Self::Malformed { .. }
            | Self::DuplicateFixtureId { .. }
            | Self::DuplicateExpectedTranslation { .. }
            | Self::PermissionDenied { .. }
            | Self::TemporarilyUnavailable { .. } => None,
        }
    }
}

impl From<MemoryReadError> for VmiFixtureError {
    fn from(value: MemoryReadError) -> Self {
        Self::Memory(value)
    }
}

impl From<RegisterReadError> for VmiFixtureError {
    fn from(value: RegisterReadError) -> Self {
        Self::Registers(value)
    }
}

impl From<ProfileError> for VmiFixtureError {
    fn from(value: ProfileError) -> Self {
        Self::Profile(value)
    }
}

impl From<TranslationError> for VmiFixtureError {
    fn from(value: TranslationError) -> Self {
        Self::Translation(value)
    }
}

#[derive(Debug, Clone)]
pub struct VmiFixture {
    pub id: String,
    pub name: String,
    pub architecture: ProfileArchitecture,
    pub memory_manifest: PathBuf,
    pub memory: OfflineGuestMemorySnapshotReader,
    pub register_fixture: PathBuf,
    pub registers: RegisterSnapshot,
    pub profile_identity: Option<ProfileIdentity>,
    pub expected_translations: Vec<ExpectedTranslation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedTranslation {
    pub name: String,
    pub mode: TranslationMode,
    pub gva: GuestVirtual,
    pub gpa: GuestPhysical,
    pub page_size: u64,
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
    pub user: bool,
}

impl ExpectedTranslation {
    pub fn result(&self) -> TranslationResult {
        TranslationResult {
            gpa: self.gpa,
            readable: self.readable,
            writable: self.writable,
            executable: self.executable,
            user: self.user,
            page_size: self.page_size,
        }
    }
}

#[derive(Default)]
struct FixtureBuilder {
    id: Option<String>,
    name: Option<String>,
    architecture: Option<ProfileArchitecture>,
    memory: Option<PathBuf>,
    registers: Option<PathBuf>,
    profile_none: bool,
    os: Option<OsKind>,
    kernel_or_build: Option<String>,
    variant: Option<String>,
    expected_translations: Vec<ExpectedTranslationEntry>,
}

#[derive(Debug, Clone)]
struct ExpectedTranslationEntry {
    line: usize,
    expected: ExpectedTranslation,
}

pub fn load_vmi_fixture(path: impl AsRef<Path>) -> Result<VmiFixture, VmiFixtureError> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|err| VmiFixtureError::TemporarilyUnavailable {
        resource: "vmi-fixture-manifest",
        detail: format!("cannot read VMI fixture manifest: {err}"),
    })?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    parse_vmi_fixture_manifest_text(&text, base_dir)
}

pub fn load_vmi_fixture_with_profiles(
    path: impl AsRef<Path>,
    profiles: &OsProfileRegistry,
) -> Result<VmiFixture, VmiFixtureError> {
    let fixture = load_vmi_fixture(path)?;
    if let Some(identity) = fixture.profile_identity.as_ref() {
        profiles.lookup(identity)?;
    }
    Ok(fixture)
}

pub fn load_vmi_fixture_set<I, P>(paths: I) -> Result<Vec<VmiFixture>, VmiFixtureError>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut seen_ids = BTreeSet::new();
    let mut fixtures = Vec::new();

    for path in paths {
        let fixture = load_vmi_fixture(path)?;
        if !seen_ids.insert(fixture.id.clone()) {
            return Err(VmiFixtureError::DuplicateFixtureId { id: fixture.id });
        }
        fixtures.push(fixture);
    }

    Ok(fixtures)
}

pub fn parse_vmi_fixture_manifest_text(
    text: &str,
    base_dir: impl AsRef<Path>,
) -> Result<VmiFixture, VmiFixtureError> {
    let base_dir = base_dir.as_ref();
    let mut lines = logical_lines(text);
    let Some((version_line, version)) = lines.next() else {
        return Err(malformed("missing VMI fixture version header"));
    };
    if version != VMI_FIXTURE_VERSION {
        return Err(malformed(format!(
            "line {version_line}: expected {VMI_FIXTURE_VERSION}"
        )));
    }

    let mut builder = FixtureBuilder::default();
    let mut singleton_keys = BTreeSet::new();
    let mut expected_names = BTreeSet::new();

    for (line, entry) in lines {
        let (key, value) = split_entry(line, entry)?;
        if key == "translation" {
            let expected = parse_expected_translation(line, value)?;
            if !expected_names.insert(expected.name.clone()) {
                return Err(VmiFixtureError::DuplicateExpectedTranslation {
                    name: expected.name,
                });
            }
            builder
                .expected_translations
                .push(ExpectedTranslationEntry { line, expected });
            continue;
        }

        reject_duplicate_key(&mut singleton_keys, line, key)?;
        parse_top_level_entry(line, key, value, base_dir, &mut builder)?;
    }

    build_fixture(builder)
}

fn build_fixture(builder: FixtureBuilder) -> Result<VmiFixture, VmiFixtureError> {
    let id = required_string(builder.id, "id")?;
    let name = required_string(builder.name, "name")?;
    let architecture = builder
        .architecture
        .ok_or_else(|| malformed("missing required fixture field 'arch'"))?;
    let memory_manifest = builder
        .memory
        .ok_or_else(|| malformed("missing required fixture field 'memory'"))?;
    let register_fixture = builder
        .registers
        .ok_or_else(|| malformed("missing required fixture field 'registers'"))?;

    if builder.profile_none
        && (builder.os.is_some() || builder.kernel_or_build.is_some() || builder.variant.is_some())
    {
        return Err(malformed(
            "profile=none cannot be combined with os, kernel_or_build, or variant",
        ));
    }

    let profile_identity = match (builder.os, builder.kernel_or_build, builder.variant) {
        (None, None, None) => None,
        (Some(os), Some(kernel_or_build), variant) => Some(ProfileIdentity::new(
            os,
            architecture.clone(),
            kernel_or_build,
            variant,
        )?),
        (None, Some(_), _) | (None, None, Some(_)) => {
            return Err(malformed(
                "profile identity fields require os and kernel_or_build",
            ));
        }
        (Some(_), None, _) => {
            return Err(malformed(
                "profile identity field 'kernel_or_build' is required when os is set",
            ));
        }
    };

    validate_expected_translations(&architecture, &builder.expected_translations)?;

    let memory = OfflineGuestMemorySnapshotReader::from_manifest(&memory_manifest)?;
    let registers = load_register_snapshot_fixture(&register_fixture)?;
    validate_register_architecture(&architecture, &registers)?;
    let expected_translations = builder
        .expected_translations
        .into_iter()
        .map(|entry| entry.expected)
        .collect();

    Ok(VmiFixture {
        id,
        name,
        architecture,
        memory_manifest,
        memory,
        register_fixture,
        registers,
        profile_identity,
        expected_translations,
    })
}

fn parse_top_level_entry(
    line: usize,
    key: &str,
    value: &str,
    base_dir: &Path,
    builder: &mut FixtureBuilder,
) -> Result<(), VmiFixtureError> {
    match key {
        "id" => builder.id = Some(normalize_required(line, key, value)?),
        "name" => builder.name = Some(normalize_required(line, key, value)?),
        "arch" => builder.architecture = Some(parse_architecture(value)?),
        "memory" => builder.memory = Some(resolve_referenced_file(line, key, value, base_dir)?),
        "registers" => {
            builder.registers = Some(resolve_referenced_file(line, key, value, base_dir)?);
        }
        "profile" => {
            if value != "none" {
                return Err(malformed(format!(
                    "line {line}: profile must be 'none' when present"
                )));
            }
            builder.profile_none = true;
        }
        "os" => builder.os = Some(parse_os(value)?),
        "kernel_or_build" => builder.kernel_or_build = Some(normalize_required(line, key, value)?),
        "variant" => builder.variant = Some(normalize_required(line, key, value)?),
        _ => {
            return Err(malformed(format!(
                "line {line}: unknown fixture key '{key}'"
            )))
        }
    }
    Ok(())
}

fn parse_expected_translation(
    line: usize,
    value: &str,
) -> Result<ExpectedTranslation, VmiFixtureError> {
    let mut seen = BTreeSet::new();
    let mut name = None;
    let mut mode = None;
    let mut gva = None;
    let mut gpa = None;
    let mut page_size = None;
    let mut readable = None;
    let mut writable = None;
    let mut executable = None;
    let mut user = None;

    for field in value.split_whitespace() {
        let (key, value) = split_field(line, field)?;
        reject_duplicate_key(&mut seen, line, key)?;
        match key {
            "name" => name = Some(normalize_required(line, key, value)?),
            "mode" => mode = Some(parse_translation_mode(value)?),
            "gva" => gva = Some(GuestVirtual(parse_u64(line, key, value)?)),
            "gpa" => gpa = Some(GuestPhysical(parse_u64(line, key, value)?)),
            "page_size" => page_size = Some(parse_page_size(line, value)?),
            "readable" => readable = Some(parse_bool(line, key, value)?),
            "writable" => writable = Some(parse_bool(line, key, value)?),
            "executable" => executable = Some(parse_bool(line, key, value)?),
            "user" => user = Some(parse_bool(line, key, value)?),
            _ => {
                return Err(malformed(format!(
                    "line {line}: unknown expected translation field '{key}'"
                )));
            }
        }
    }

    let mode = mode.ok_or_else(|| malformed(line_missing(line, "translation.mode")))?;
    let page_size =
        page_size.ok_or_else(|| malformed(line_missing(line, "translation.page_size")))?;
    validate_mode_page_size(line, mode, page_size)?;

    Ok(ExpectedTranslation {
        name: required_string(name, "translation.name")?,
        mode,
        gva: gva.ok_or_else(|| malformed(line_missing(line, "translation.gva")))?,
        gpa: gpa.ok_or_else(|| malformed(line_missing(line, "translation.gpa")))?,
        page_size,
        readable: readable.ok_or_else(|| malformed(line_missing(line, "translation.readable")))?,
        writable: writable.ok_or_else(|| malformed(line_missing(line, "translation.writable")))?,
        executable: executable
            .ok_or_else(|| malformed(line_missing(line, "translation.executable")))?,
        user: user.ok_or_else(|| malformed(line_missing(line, "translation.user")))?,
    })
}

fn validate_register_architecture(
    architecture: &ProfileArchitecture,
    registers: &RegisterSnapshot,
) -> Result<(), VmiFixtureError> {
    match (architecture, registers) {
        (ProfileArchitecture::X86_64, RegisterSnapshot::X86_64(_))
        | (ProfileArchitecture::Arm64, RegisterSnapshot::Arm64(_)) => Ok(()),
        (ProfileArchitecture::X86_64, RegisterSnapshot::Arm64(_)) => {
            Err(RegisterReadError::WrongArchitecture {
                expected: "x86_64",
                actual: "arm64",
            }
            .into())
        }
        (ProfileArchitecture::Arm64, RegisterSnapshot::X86_64(_)) => {
            Err(RegisterReadError::WrongArchitecture {
                expected: "arm64",
                actual: "x86_64",
            }
            .into())
        }
        (_, RegisterSnapshot::UnsupportedArchitecture { arch }) => {
            Err(RegisterReadError::UnsupportedArchitecture { arch: arch.clone() }.into())
        }
        (ProfileArchitecture::Other(arch), _) => {
            Err(ProfileError::UnsupportedArchitecture { arch: arch.clone() }.into())
        }
    }
}

fn validate_expected_translations(
    architecture: &ProfileArchitecture,
    translations: &[ExpectedTranslationEntry],
) -> Result<(), VmiFixtureError> {
    for entry in translations {
        validate_mode_architecture(entry.line, architecture, entry.expected.mode)?;
    }
    Ok(())
}

fn validate_mode_architecture(
    line: usize,
    architecture: &ProfileArchitecture,
    mode: TranslationMode,
) -> Result<(), VmiFixtureError> {
    match (architecture, mode) {
        (
            ProfileArchitecture::X86_64,
            TranslationMode::X86_64FourLevel | TranslationMode::X86_64La57,
        )
        | (ProfileArchitecture::Arm64, TranslationMode::Arm64Stage1 { .. }) => Ok(()),
        (ProfileArchitecture::X86_64, TranslationMode::Arm64Stage1 { .. }) => {
            Err(malformed(format!(
                "line {line}: translation mode '{}' does not match fixture arch 'x86_64'",
                translation_mode_name(mode)
            )))
        }
        (
            ProfileArchitecture::Arm64,
            TranslationMode::X86_64FourLevel | TranslationMode::X86_64La57,
        ) => Err(malformed(format!(
            "line {line}: translation mode '{}' does not match fixture arch 'arm64'",
            translation_mode_name(mode)
        ))),
        (ProfileArchitecture::Other(arch), _) => {
            Err(ProfileError::UnsupportedArchitecture { arch: arch.clone() }.into())
        }
    }
}

fn validate_mode_page_size(
    line: usize,
    mode: TranslationMode,
    page_size: u64,
) -> Result<(), VmiFixtureError> {
    let valid = match mode {
        TranslationMode::X86_64FourLevel | TranslationMode::X86_64La57 => {
            matches!(page_size, 0x1000 | 0x20_0000 | 0x4000_0000)
        }
        TranslationMode::Arm64Stage1 {
            granule: Arm64CacheGranule::Size4K,
        } => matches!(page_size, 0x1000 | 0x20_0000 | 0x4000_0000),
        TranslationMode::Arm64Stage1 {
            granule: Arm64CacheGranule::Size16K,
        } => matches!(page_size, 0x4000 | 0x200_0000 | 0x0010_0000_0000),
        TranslationMode::Arm64Stage1 {
            granule: Arm64CacheGranule::Size64K,
        } => matches!(page_size, 0x1_0000 | 0x2000_0000),
    };

    if valid {
        Ok(())
    } else {
        Err(malformed(format!(
            "line {line}: page_size 0x{page_size:x} is not valid for translation mode '{}'",
            translation_mode_name(mode)
        )))
    }
}

pub fn translation_mode_name(mode: TranslationMode) -> &'static str {
    match mode {
        TranslationMode::X86_64FourLevel => "x86_64-4level",
        TranslationMode::X86_64La57 => "x86_64-la57",
        TranslationMode::Arm64Stage1 {
            granule: Arm64CacheGranule::Size4K,
        } => "arm64-stage1-4k",
        TranslationMode::Arm64Stage1 {
            granule: Arm64CacheGranule::Size16K,
        } => "arm64-stage1-16k",
        TranslationMode::Arm64Stage1 {
            granule: Arm64CacheGranule::Size64K,
        } => "arm64-stage1-64k",
    }
}

fn parse_os(value: &str) -> Result<OsKind, VmiFixtureError> {
    match value {
        "linux" => Ok(OsKind::Linux),
        "windows" => Ok(OsKind::Windows),
        other => Err(ProfileError::UnsupportedGuest {
            os: other.to_string(),
            arch: "*".to_string(),
        }
        .into()),
    }
}

fn parse_architecture(value: &str) -> Result<ProfileArchitecture, VmiFixtureError> {
    match value {
        "x86_64" => Ok(ProfileArchitecture::X86_64),
        "arm64" => Ok(ProfileArchitecture::Arm64),
        other => Err(ProfileError::UnsupportedArchitecture {
            arch: other.to_string(),
        }
        .into()),
    }
}

pub fn parse_translation_mode(value: &str) -> Result<TranslationMode, VmiFixtureError> {
    match value {
        "x86_64-4level" => Ok(TranslationMode::X86_64FourLevel),
        "x86_64-la57" => Ok(TranslationMode::X86_64La57),
        "arm64-stage1-4k" => Ok(TranslationMode::Arm64Stage1 {
            granule: Arm64CacheGranule::Size4K,
        }),
        "arm64-stage1-16k" => Ok(TranslationMode::Arm64Stage1 {
            granule: Arm64CacheGranule::Size16K,
        }),
        "arm64-stage1-64k" => Ok(TranslationMode::Arm64Stage1 {
            granule: Arm64CacheGranule::Size64K,
        }),
        _ => Err(TranslationError::Unsupported {
            backend: "vmi-fixture",
            operation: "expected_translation_mode",
        }
        .into()),
    }
}

fn resolve_referenced_file(
    line: usize,
    field: &str,
    value: &str,
    base_dir: &Path,
) -> Result<PathBuf, VmiFixtureError> {
    let relative = parse_relative_path(line, field, value)?;
    let full = base_dir.join(&relative);
    if !full.is_file() {
        return match field {
            "memory" => Err(MemoryReadError::TemporarilyUnavailable {
                resource: "vmi-fixture-memory-manifest",
                detail: format!(
                    "line {line}: memory manifest '{}' is missing",
                    relative.display()
                ),
            }
            .into()),
            "registers" => Err(RegisterReadError::TemporarilyUnavailable {
                resource: "vmi-fixture-registers",
                detail: format!(
                    "line {line}: register fixture '{}' is missing",
                    relative.display()
                ),
            }
            .into()),
            _ => Err(malformed(format!(
                "line {line}: referenced file '{}' is missing",
                relative.display()
            ))),
        };
    }
    Ok(full)
}

fn parse_relative_path(line: usize, field: &str, value: &str) -> Result<PathBuf, VmiFixtureError> {
    let path = Path::new(value);
    if value.contains('\\')
        || value.contains(':')
        || value.starts_with("//")
        || value.starts_with("\\\\")
    {
        return Err(VmiFixtureError::PermissionDenied {
            operation: "load_vmi_fixture",
            detail: format!("line {line}: field '{field}' must use a portable relative path"),
        });
    }

    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(VmiFixtureError::PermissionDenied {
            operation: "load_vmi_fixture",
            detail: format!("line {line}: field '{field}' must use a relative path"),
        });
    }

    // Fixture manifests are portable only when references stay below the manifest directory.
    if !path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(VmiFixtureError::PermissionDenied {
            operation: "load_vmi_fixture",
            detail: format!("line {line}: field '{field}' must not escape the manifest directory"),
        });
    }

    Ok(path.to_path_buf())
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

fn split_entry(line: usize, entry: &str) -> Result<(&str, &str), VmiFixtureError> {
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

fn split_field(line: usize, field: &str) -> Result<(&str, &str), VmiFixtureError> {
    split_entry(line, field)
}

fn reject_duplicate_key<'a>(
    seen: &mut BTreeSet<&'a str>,
    line: usize,
    key: &'a str,
) -> Result<(), VmiFixtureError> {
    if !seen.insert(key) {
        return Err(malformed(format!(
            "line {line}: duplicate fixture key '{key}'"
        )));
    }
    Ok(())
}

fn required_string(value: Option<String>, field: &str) -> Result<String, VmiFixtureError> {
    value.ok_or_else(|| malformed(format!("missing required fixture field '{field}'")))
}

fn normalize_required(line: usize, field: &str, value: &str) -> Result<String, VmiFixtureError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(malformed(format!(
            "line {line}: field '{field}' must not be empty"
        )));
    }
    Ok(value.to_string())
}

fn parse_u64(line: usize, field: &str, value: &str) -> Result<u64, VmiFixtureError> {
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

fn parse_page_size(line: usize, value: &str) -> Result<u64, VmiFixtureError> {
    let page_size = parse_u64(line, "page_size", value)?;
    if page_size == 0 || !page_size.is_power_of_two() {
        return Err(malformed(format!(
            "line {line}: page_size must be a non-zero power of two"
        )));
    }
    Ok(page_size)
}

fn parse_bool(line: usize, field: &str, value: &str) -> Result<bool, VmiFixtureError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(malformed(format!(
            "line {line}: invalid boolean for field '{field}': {value}"
        ))),
    }
}

fn line_missing(line: usize, field: &str) -> String {
    format!("line {line}: missing required fixture field '{field}'")
}

fn malformed(detail: impl Into<String>) -> VmiFixtureError {
    VmiFixtureError::Malformed {
        detail: detail.into(),
    }
}
