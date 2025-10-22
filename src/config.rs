use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub general: General,
    pub allow: Allow,
    pub pmu: Pmu,
}
#[derive(Debug, Clone, Deserialize)]
pub struct General { pub wx_window_ms: u64, pub quiet: bool }
#[derive(Debug, Clone, Deserialize)]
pub struct Allow { pub names: Vec<String> }
#[derive(Debug, Clone, Deserialize)]
pub struct Pmu { pub enable: bool, pub sample_ms: u64 }

impl Default for Config {
    fn default() -> Self {
        Self {
            general: General { wx_window_ms: 5000, quiet: false },
            allow: Allow { names: vec![] },
            pmu: Pmu { enable: true, sample_ms: 1000 },
        }
    }
}

impl Config {
    pub fn load(path: Option<&std::path::Path>) -> Option<Self> {
        if let Some(p) = path {
            let s = std::fs::read_to_string(p).ok()?;
            toml::from_str(&s).ok()
        } else { None }
    }
}
