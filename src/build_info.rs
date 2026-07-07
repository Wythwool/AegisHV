use crate::util::json_str;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    pub version: &'static str,
    pub target_os: &'static str,
    pub target_arch: &'static str,
    pub git_rev: &'static str,
}

impl BuildInfo {
    pub fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            target_os: std::env::consts::OS,
            target_arch: std::env::consts::ARCH,
            git_rev: option_env!("AEGISHV_BUILD_GIT_REV").unwrap_or("unknown"),
        }
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\"version\":{},\"target_os\":{},\"target_arch\":{},\"git_rev\":{}}}",
            json_str(self.version),
            json_str(self.target_os),
            json_str(self.target_arch),
            json_str(self.git_rev)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_info_reports_crate_version_and_target() {
        let info = BuildInfo::current();

        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert!(!info.target_os.is_empty());
        assert!(info.to_json().contains("\"version\""));
    }
}
