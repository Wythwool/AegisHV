use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::vmi::ProfileError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OsKind {
    Linux,
    Windows,
    Other(String),
}

impl OsKind {
    pub fn linux() -> Self {
        Self::Linux
    }

    pub fn windows() -> Self {
        Self::Windows
    }

    pub fn other(name: impl Into<String>) -> Self {
        Self::Other(name.into())
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Linux => "linux",
            Self::Windows => "windows",
            Self::Other(name) => name.as_str(),
        }
    }

    fn validate_supported(&self) -> Result<(), ProfileError> {
        match self {
            Self::Linux | Self::Windows => Ok(()),
            Self::Other(os) => Err(ProfileError::UnsupportedGuest {
                os: os.clone(),
                arch: "*".to_string(),
            }),
        }
    }
}

impl fmt::Display for OsKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProfileArchitecture {
    X86_64,
    Arm64,
    Other(String),
}

impl ProfileArchitecture {
    pub fn x86_64() -> Self {
        Self::X86_64
    }

    pub fn arm64() -> Self {
        Self::Arm64
    }

    pub fn other(name: impl Into<String>) -> Self {
        Self::Other(name.into())
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Arm64 => "arm64",
            Self::Other(name) => name.as_str(),
        }
    }

    fn validate_supported(&self) -> Result<(), ProfileError> {
        match self {
            Self::X86_64 | Self::Arm64 => Ok(()),
            Self::Other(arch) => Err(ProfileError::UnsupportedArchitecture { arch: arch.clone() }),
        }
    }
}

impl fmt::Display for ProfileArchitecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProfileIdentity {
    pub os: OsKind,
    pub arch: ProfileArchitecture,
    pub kernel_or_build: String,
    pub variant: Option<String>,
}

impl ProfileIdentity {
    pub fn new(
        os: OsKind,
        arch: ProfileArchitecture,
        kernel_or_build: impl Into<String>,
        variant: Option<impl Into<String>>,
    ) -> Result<Self, ProfileError> {
        os.validate_supported()?;
        arch.validate_supported()?;
        let kernel_or_build = normalize_required("kernel_or_build", kernel_or_build.into())?;
        let variant = variant
            .map(Into::into)
            .map(|value| normalize_required("variant", value))
            .transpose()?;

        Ok(Self {
            os,
            arch,
            kernel_or_build,
            variant,
        })
    }

    pub fn linux_x86_64(kernel_release: impl Into<String>) -> Result<Self, ProfileError> {
        Self::new(
            OsKind::Linux,
            ProfileArchitecture::X86_64,
            kernel_release,
            None::<String>,
        )
    }

    pub fn windows_x86_64(build: impl Into<String>) -> Result<Self, ProfileError> {
        Self::new(
            OsKind::Windows,
            ProfileArchitecture::X86_64,
            build,
            None::<String>,
        )
    }
}

pub trait OsProfile: fmt::Debug + Send + Sync {
    fn identity(&self) -> &ProfileIdentity;
    fn profile_name(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct StaticOsProfile {
    identity: ProfileIdentity,
    profile_name: String,
}

impl StaticOsProfile {
    pub fn synthetic(
        identity: ProfileIdentity,
        profile_name: impl Into<String>,
    ) -> Result<Self, ProfileError> {
        Ok(Self {
            identity,
            profile_name: normalize_required("profile_name", profile_name.into())?,
        })
    }
}

impl OsProfile for StaticOsProfile {
    fn identity(&self) -> &ProfileIdentity {
        &self.identity
    }

    fn profile_name(&self) -> &str {
        &self.profile_name
    }
}

#[derive(Default)]
pub struct OsProfileRegistry {
    profiles: BTreeMap<ProfileIdentity, Arc<dyn OsProfile>>,
}

impl OsProfileRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    pub fn register<P>(&mut self, profile: P) -> Result<(), ProfileError>
    where
        P: OsProfile + 'static,
    {
        let identity = profile.identity().clone();
        validate_identity(&identity)?;
        if self.profiles.contains_key(&identity) {
            return Err(ProfileError::MalformedProfile {
                detail: format!(
                    "duplicate profile key os='{}' arch='{}' kernel_or_build='{}'",
                    identity.os, identity.arch, identity.kernel_or_build
                ),
            });
        }

        self.profiles.insert(identity, Arc::new(profile));
        Ok(())
    }

    pub fn lookup(&self, identity: &ProfileIdentity) -> Result<Arc<dyn OsProfile>, ProfileError> {
        validate_identity(identity)?;
        self.profiles
            .get(identity)
            .cloned()
            .ok_or_else(|| ProfileError::MissingProfileIdentity {
                os: identity.os.to_string(),
                arch: identity.arch.to_string(),
                kernel_or_build: identity.kernel_or_build.clone(),
            })
    }
}

fn validate_identity(identity: &ProfileIdentity) -> Result<(), ProfileError> {
    identity.os.validate_supported()?;
    identity.arch.validate_supported()?;
    normalize_required("kernel_or_build", identity.kernel_or_build.clone())?;
    if let Some(variant) = identity.variant.as_ref() {
        normalize_required("variant", variant.clone())?;
    }
    Ok(())
}

fn normalize_required(field: &'static str, value: String) -> Result<String, ProfileError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ProfileError::MalformedProfile {
            detail: format!("profile identity field '{field}' must not be empty"),
        });
    }
    Ok(trimmed.to_string())
}
