use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const CLAIM_TERMS: &[&str] = &[
    "type-1",
    "full vmi",
    "ept/npt",
    "stage-2 enforcement",
    "syscall-path integrity",
    "hardware pmu sampling",
    "libvirt lifecycle",
    "finished edr",
    "production",
];

const HONEST_CONTEXT: &[&str] = &[
    "not",
    "unsupported",
    "without",
    "unless",
    "before",
    "roadmap",
    "planned",
    "gate",
    "must",
    "cannot",
    "disabled",
    "required",
    "still",
    "do not",
    "separate",
    "future",
    "target",
    "claim",
    "limits",
    "boundary",
    "boundaries",
    "fallback",
    "unavailable",
    "not implemented",
];

const RELEASE_CLAIM: &str = concat!("production", "-", "ready");

const FORBIDDEN_PHRASES: &[&str] = &[
    RELEASE_CLAIM,
    concat!("enterprise", "-", "grade"),
    concat!("next", "-", "generation"),
    concat!("ai", "-", "powered"),
    concat!("robust", " ", "solution"),
    concat!("comprehensive", " ", "approach"),
    concat!("sea", "mless"),
    concat!("left", " ", "as", " ", "an", " ", "exercise"),
    concat!("TO", "DO"),
    concat!("FIX", "ME"),
    concat!("T", "BD"),
];

#[derive(Debug, PartialEq, Eq)]
enum ClaimDocError {
    MissingFile(&'static str),
    MissingClaimDiscipline,
    MissingClaimDisciplineText(&'static str),
    MisleadingClaim {
        path: String,
        line: usize,
        text: String,
    },
    ForbiddenPhrase {
        path: String,
        phrase: &'static str,
    },
    ReadDir(String),
    ReadFile(String),
}

impl fmt::Display for ClaimDocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClaimDocError::MissingFile(path) => write!(f, "{path} is missing"),
            ClaimDocError::MissingClaimDiscipline => {
                write!(f, "docs/STATUS.md is missing claim discipline")
            }
            ClaimDocError::MissingClaimDisciplineText(text) => {
                write!(f, "claim discipline is missing required text: {text}")
            }
            ClaimDocError::MisleadingClaim { path, line, text } => {
                write!(f, "{path}:{line} has an unsupported claim: {text}")
            }
            ClaimDocError::ForbiddenPhrase { path, phrase } => {
                write!(f, "{path} contains forbidden phrase: {phrase}")
            }
            ClaimDocError::ReadDir(path) => write!(f, "could not read directory: {path}"),
            ClaimDocError::ReadFile(path) => write!(f, "could not read file: {path}"),
        }
    }
}

fn read_required_file(root: &Path, rel: &'static str) -> Result<String, ClaimDocError> {
    fs::read_to_string(root.join(rel)).map_err(|_| ClaimDocError::MissingFile(rel))
}

fn markdown_files(root: &Path) -> Result<Vec<PathBuf>, ClaimDocError> {
    let mut files = vec![root.join("README.md"), root.join("BACKLOG.md")];
    collect_markdown(&root.join("docs"), &mut files)?;
    Ok(files)
}

fn collect_markdown(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), ClaimDocError> {
    let entries =
        fs::read_dir(dir).map_err(|_| ClaimDocError::ReadDir(dir.display().to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|_| ClaimDocError::ReadDir(dir.display().to_string()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_markdown(&path, files)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some("md") {
            files.push(path);
        }
    }
    Ok(())
}

fn validate_status_claim_discipline(text: &str) -> Result<(), ClaimDocError> {
    if !text.contains("## Claim discipline") {
        return Err(ClaimDocError::MissingClaimDiscipline);
    }

    for required in [
        "Linux host-side KVM telemetry sensor",
        "Do not describe this tree as a production or general-purpose Type-1 hypervisor",
        "hardware PMU sampling",
        "Roadmap documents may discuss those targets",
    ] {
        if !text.contains(required) {
            return Err(ClaimDocError::MissingClaimDisciplineText(required));
        }
    }

    Ok(())
}

fn validate_docs_claims(root: &Path) -> Result<(), ClaimDocError> {
    for path in markdown_files(root)? {
        let text = fs::read_to_string(&path)
            .map_err(|_| ClaimDocError::ReadFile(path.display().to_string()))?;
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string()
            .replace('\\', "/");
        reject_forbidden_phrases(&rel, &text)?;
        for (idx, line) in text.lines().enumerate() {
            validate_claim_line(&rel, idx + 1, line)?;
        }
    }
    Ok(())
}

fn assert_contains_all(text: &str, required: &[&str]) {
    for item in required {
        assert!(text.contains(item), "missing required VMI doc text: {item}");
    }
}

fn validate_claim_line(path: &str, line_no: usize, line: &str) -> Result<(), ClaimDocError> {
    let lower = line.to_ascii_lowercase();
    if !CLAIM_TERMS.iter().any(|term| lower.contains(term)) {
        return Ok(());
    }

    if is_roadmap_context(path) || HONEST_CONTEXT.iter().any(|token| lower.contains(token)) {
        return Ok(());
    }

    Err(ClaimDocError::MisleadingClaim {
        path: path.to_string(),
        line: line_no,
        text: line.to_string(),
    })
}

fn reject_forbidden_phrases(path: &str, text: &str) -> Result<(), ClaimDocError> {
    let lower = text.to_ascii_lowercase();
    for &phrase in FORBIDDEN_PHRASES {
        if lower.contains(&phrase.to_ascii_lowercase()) {
            return Err(ClaimDocError::ForbiddenPhrase {
                path: path.to_string(),
                phrase,
            });
        }
    }
    Ok(())
}

fn is_roadmap_context(path: &str) -> bool {
    path == "BACKLOG.md" || path.ends_with("TYPE1_ROADMAP.md")
}

#[test]
fn status_claim_discipline_is_present() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = read_required_file(root, "docs/STATUS.md").expect("status doc must exist");

    validate_status_claim_discipline(&text).expect("status doc must define claim discipline");
}

#[test]
fn docs_keep_unsupported_claims_in_honest_context() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    validate_docs_claims(root).expect("docs must not imply unsupported capabilities");
}

#[test]
fn vmi_safety_doc_records_offline_boundaries_and_live_read_requirements() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = read_required_file(root, "docs/VMI.md").expect("VMI safety doc must exist");

    assert_contains_all(
        &text,
        &[
            "typed VMI errors",
            "synthetic guest physical memory ranges",
            "read-only offline memory snapshot manifests",
            "x86_64 4-level guest virtual to guest physical translation",
            "x86_64 LA57 5-level translation",
            "ARM64 stage-1 translation for 4 KiB, 16 KiB, and 64 KiB granules",
            "translation cache infrastructure",
            "architecture-neutral register snapshots",
            "offline register fixture loading",
            "OS profile identity and registry types",
            "VMI fixture loading",
            "offline `vmi translate` CLI",
            "VMI metrics skeleton counters",
        ],
    );

    assert_contains_all(
        &text,
        &[
            "Live VMI backend support is not implemented.",
            "Live guest memory reads are not implemented.",
            "Live guest register reads are not implemented.",
            "Full VMI stack behavior is not implemented.",
            "Direct EPT/NPT/Stage-2 enforcement is not implemented.",
            "Type-1 runtime support is not implemented.",
            "Syscall integrity implementation is not present.",
            "Hardware PMU sampling is not implemented.",
            "Production guest inspection is not implemented.",
            "Real Linux or Windows OS profile data is not shipped.",
        ],
    );

    assert_contains_all(
        &text,
        &[
            "must describe the same guest point-in-time",
            "cannot prove that fixture bytes or register values are semantically true",
            "Live page-table walks can race CR3 or TTBR changes",
            "AegisHV does not claim a bypass",
            "stable VM identity",
            "page-table read consistency rules",
            "Partial reads must fail",
            "fail closed with typed translation errors",
            "does not ship real Linux or Windows profile data",
            "There is no nearest-match fallback.",
            "Absolute paths, Windows drive paths, UNC-like paths",
            "Failed translations are not cached.",
            "must not expose raw GVA or GPA values",
        ],
    );
}

#[test]
fn linux_vmi_profile_doc_records_synthetic_only_scope() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = read_required_file(root, "docs/VMI_LINUX.md").expect("Linux VMI doc must exist");

    assert_contains_all(
        &text,
        &[
            "synthetic/offline and x86_64-only",
            "parses kallsyms/System.map-style symbol maps",
            "No real Linux kernel profile data ships by default.",
            "bounded profile anchors",
            "Duplicate names are preserved",
            "`task_struct` list walking",
            "syscall table handler inspection",
            "ftrace callback inventory",
            "kprobe target and handler inventory",
            "BPF program inventory and bounded JIT ranges",
            "off-hot-path Linux detector runner",
            "W^X events preserve guest attribution fields",
            "There is no live Linux guest backend.",
            "Lookup is still exact",
            "There is no nearest-match fallback.",
            "Duplicate symbol names",
            "duplicate syscall numbers",
        ],
    );
}

#[test]
fn windows_vmi_profile_doc_records_synthetic_only_scope() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = read_required_file(root, "docs/VMI_WINDOWS.md").expect("Windows VMI doc must exist");
    let symbols = read_required_file(root, "docs/VMI_WINDOWS_SYMBOLS.md")
        .expect("Windows symbol doc must exist");

    assert_contains_all(
        &text,
        &[
            "synthetic/offline and x86_64-only",
            "exact build and PDB identity",
            "No real Windows profile data ships by default.",
            "There is no nearest-match fallback.",
            "ntoskrnl base resolution",
            "`EPROCESS` list walking",
            "SSDT handler inspection",
            "`MSR_LSTAR` inspection",
            "process-create callback inventory",
            "kernel and driver text hashing",
            "protection-limit reporting",
            "off-hot-path Windows detector runner",
            "There is no live Windows guest backend.",
            "Real Windows profile extraction is not shipped.",
            "AegisHV does not claim a bypass.",
        ],
    );

    assert_contains_all(
        &symbols,
        &[
            "offline metadata format for pre-extracted kernel symbols",
            "It is not a downloader",
            "does not contact the network",
            "Exact PDB identity still matters",
            "No real Windows symbol cache data ships by default.",
            "There is no automatic symbol download.",
            "There is no PDB type reconstruction.",
        ],
    );
}

#[test]
fn detector_docs_record_engine_limits_and_mapping_scope() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let engine =
        read_required_file(root, "docs/DETECTOR_ENGINE.md").expect("detector doc must exist");
    let limits =
        read_required_file(root, "docs/DETECTION_LIMITS.md").expect("limits doc must exist");
    let mitre = read_required_file(root, "docs/MITRE_MAPPING.md").expect("MITRE doc must exist");

    assert_contains_all(
        &engine,
        &[
            "detector engine library layer",
            "`Detector` trait",
            "Per-detector runtime and finding-count budgets",
            "Severity and confidence scoring",
            "Versioned detector state file",
            "not a live guest backend",
            "No public event schema changed",
            "does not preempt a running detector thread",
        ],
    );
    assert_contains_all(
        &limits,
        &[
            "`kernel_text_tamper`",
            "`syscall_hook`",
            "`hidden_process`",
            "`hidden_module`",
            "`executable_anonymous_memory`",
            "`rwx_mapping`",
            "`wx_correlation`",
            "False positives",
            "False negatives",
            "Unsupported cases",
        ],
    );
    assert_contains_all(
        &mitre,
        &[
            "implemented detection records",
            "`kernel_text_tamper`",
            "T1014 Rootkit",
            "`executable_anonymous_memory`",
            "T1055 Process Injection",
            "No mapping is provided for detectors that are unsupported",
        ],
    );
}

#[test]
fn claim_line_validator_rejects_unqualified_full_vmi_claim() {
    assert_eq!(
        validate_claim_line("docs/STATUS.md", 7, "AegisHV provides full VMI support."),
        Err(ClaimDocError::MisleadingClaim {
            path: "docs/STATUS.md".to_string(),
            line: 7,
            text: "AegisHV provides full VMI support.".to_string(),
        })
    );
}

#[test]
fn claim_line_validator_accepts_unsupported_context() {
    assert_eq!(
        validate_claim_line("docs/STATUS.md", 7, "Full VMI is not implemented."),
        Ok(())
    );
}

#[test]
fn claim_line_validator_accepts_roadmap_context() {
    assert_eq!(
        validate_claim_line("docs/TYPE1_ROADMAP.md", 7, "Implement full VMI support."),
        Ok(())
    );
}
