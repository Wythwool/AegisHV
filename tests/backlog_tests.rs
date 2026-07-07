use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::Path;

const REQUIRED_PHASES: &[&str] = &[
    "Phase 0", "Phase 1", "Phase 2", "Phase 3", "Phase 4", "Phase 5", "Phase 6",
];

const REQUIRED_TOPICS: &[&str] = &[
    "Host Sensor Cleanup",
    "VMI Foundation",
    "Trap Engine",
    "Intel VMX",
    "AMD SVM",
    "ARM64 EL2",
    "IOMMU",
    "Device",
    "Management",
    "Release",
    "Hardware",
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

const VALID_BACKLOG_MINIMUM: &str = r#"
# Backlog

## Phase 0 - Host Sensor Cleanup

### B001 - Host sensor contract

- Status: open
- Scope: Keep host-side behavior explicit.
- Acceptance criteria: tests cover success and refusal paths.
- Production gate: release evidence records the command output.

## Phase 1 - VMI Foundation

## Phase 2 - Trap Engine

## Phase 3 - Intel VMX, AMD SVM, and ARM64 EL2

## Phase 4 - IOMMU and Device Boundaries

## Phase 5 - Management

## Phase 6 - Release and Hardware Tests
"#;

#[derive(Debug, PartialEq, Eq)]
enum BacklogDocError {
    MissingFile(&'static str),
    EmptyDocument,
    MissingPhase(&'static str),
    MissingTopic(&'static str),
    MissingItems,
    MalformedItemHeader(String),
    DuplicateId(String),
    MissingItemField {
        id: String,
        field: &'static str,
    },
    UnsupportedCompleteStatus(String),
    ForbiddenPhrase(&'static str),
    MissingDocLink {
        file: &'static str,
        target: &'static str,
    },
}

impl fmt::Display for BacklogDocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BacklogDocError::MissingFile(path) => write!(f, "{path} is missing"),
            BacklogDocError::EmptyDocument => write!(f, "BACKLOG.md is empty"),
            BacklogDocError::MissingPhase(phase) => write!(f, "BACKLOG.md is missing {phase}"),
            BacklogDocError::MissingTopic(topic) => {
                write!(f, "BACKLOG.md is missing required topic: {topic}")
            }
            BacklogDocError::MissingItems => write!(f, "BACKLOG.md has no backlog items"),
            BacklogDocError::MalformedItemHeader(header) => {
                write!(
                    f,
                    "backlog item header must start with an ID like B001: {header}"
                )
            }
            BacklogDocError::DuplicateId(id) => write!(f, "BACKLOG.md repeats item ID {id}"),
            BacklogDocError::MissingItemField { id, field } => {
                write!(f, "backlog item {id} is missing field: {field}")
            }
            BacklogDocError::UnsupportedCompleteStatus(id) => {
                write!(
                    f,
                    "backlog item {id} is marked complete without release evidence"
                )
            }
            BacklogDocError::ForbiddenPhrase(phrase) => {
                write!(f, "BACKLOG.md contains forbidden phrase: {phrase}")
            }
            BacklogDocError::MissingDocLink { file, target } => {
                write!(f, "{file} does not link to {target}")
            }
        }
    }
}

#[derive(Default)]
struct ParsedItem {
    id: String,
    status: Option<String>,
    scope: bool,
    acceptance: bool,
    gate: bool,
}

fn read_required_file(root: &Path, rel: &'static str) -> Result<String, BacklogDocError> {
    fs::read_to_string(root.join(rel)).map_err(|_| BacklogDocError::MissingFile(rel))
}

fn validate_backlog_doc(text: &str) -> Result<(), BacklogDocError> {
    if text.trim().is_empty() {
        return Err(BacklogDocError::EmptyDocument);
    }

    let lower = text.to_ascii_lowercase();
    for &phrase in FORBIDDEN_PHRASES {
        if lower.contains(&phrase.to_ascii_lowercase()) {
            return Err(BacklogDocError::ForbiddenPhrase(phrase));
        }
    }

    for &phase in REQUIRED_PHASES {
        if !text.contains(phase) {
            return Err(BacklogDocError::MissingPhase(phase));
        }
    }

    for &topic in REQUIRED_TOPICS {
        if !text.contains(topic) {
            return Err(BacklogDocError::MissingTopic(topic));
        }
    }

    let mut seen = HashSet::new();
    let mut items = Vec::new();
    let mut current: Option<ParsedItem> = None;

    for line in text.lines() {
        if let Some(header) = line.strip_prefix("### ") {
            if let Some(item) = current.take() {
                items.push(item);
            }
            let id = header.split_whitespace().next().unwrap_or_default();
            if !is_backlog_id(id) {
                return Err(BacklogDocError::MalformedItemHeader(header.to_string()));
            }
            if !seen.insert(id.to_string()) {
                return Err(BacklogDocError::DuplicateId(id.to_string()));
            }
            current = Some(ParsedItem {
                id: id.to_string(),
                ..ParsedItem::default()
            });
            continue;
        }

        if let Some(item) = current.as_mut() {
            if let Some(value) = line.strip_prefix("- Status: ") {
                item.status = Some(value.trim().to_string());
            } else if line.starts_with("- Scope: ") {
                item.scope = true;
            } else if line.starts_with("- Acceptance criteria: ") {
                item.acceptance = true;
            } else if line.starts_with("- Production gate: ") {
                item.gate = true;
            }
        }
    }

    if let Some(item) = current {
        items.push(item);
    }

    if items.is_empty() {
        return Err(BacklogDocError::MissingItems);
    }

    for item in items {
        if matches!(
            item.status.as_deref(),
            Some("done" | "complete" | "completed")
        ) {
            return Err(BacklogDocError::UnsupportedCompleteStatus(item.id));
        }
        if item.status.is_none() {
            return Err(BacklogDocError::MissingItemField {
                id: item.id,
                field: "Status",
            });
        }
        if !item.scope {
            return Err(BacklogDocError::MissingItemField {
                id: item.id,
                field: "Scope",
            });
        }
        if !item.acceptance {
            return Err(BacklogDocError::MissingItemField {
                id: item.id,
                field: "Acceptance criteria",
            });
        }
        if !item.gate {
            return Err(BacklogDocError::MissingItemField {
                id: item.id,
                field: "Production gate",
            });
        }
    }

    Ok(())
}

fn validate_backlog_links(root: &Path) -> Result<(), BacklogDocError> {
    let readme = read_required_file(root, "README.md")?;
    if !readme.contains("BACKLOG.md") {
        return Err(BacklogDocError::MissingDocLink {
            file: "README.md",
            target: "BACKLOG.md",
        });
    }

    let roadmap = read_required_file(root, "docs/TYPE1_ROADMAP.md")?;
    if !roadmap.contains("BACKLOG.md") {
        return Err(BacklogDocError::MissingDocLink {
            file: "docs/TYPE1_ROADMAP.md",
            target: "BACKLOG.md",
        });
    }

    Ok(())
}

fn is_backlog_id(value: &str) -> bool {
    let Some(rest) = value.strip_prefix('B') else {
        return false;
    };
    rest.len() == 3 && rest.bytes().all(|b| b.is_ascii_digit())
}

#[test]
fn backlog_file_has_phase_items_and_local_doc_links() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = read_required_file(root, "BACKLOG.md")
        .expect("BACKLOG.md must exist at the repository root");

    validate_backlog_doc(&text).expect("BACKLOG.md must keep the phase backlog contract");
    validate_backlog_links(root).expect("README and type-1 roadmap must link to BACKLOG.md");
}

#[test]
fn backlog_validator_accepts_required_item_fields() {
    assert_eq!(validate_backlog_doc(VALID_BACKLOG_MINIMUM), Ok(()));
}

#[test]
fn backlog_validator_rejects_missing_production_gate() {
    let text = VALID_BACKLOG_MINIMUM.replace(
        "- Production gate: release evidence records the command output.\n",
        "",
    );

    assert_eq!(
        validate_backlog_doc(&text),
        Err(BacklogDocError::MissingItemField {
            id: "B001".to_string(),
            field: "Production gate",
        })
    );
}

#[test]
fn backlog_validator_rejects_malformed_item_header() {
    let text = VALID_BACKLOG_MINIMUM.replace(
        "### B001 - Host sensor contract",
        "### Host sensor contract",
    );

    assert_eq!(
        validate_backlog_doc(&text),
        Err(BacklogDocError::MalformedItemHeader(
            "Host sensor contract".to_string()
        ))
    );
}

#[test]
fn backlog_validator_rejects_duplicate_ids() {
    let text = format!(
        "{VALID_BACKLOG_MINIMUM}\n### B001 - Duplicate\n\n- Status: open\n- Scope: Duplicate item.\n- Acceptance criteria: rejected by parser.\n- Production gate: no release gate until fixed.\n"
    );

    assert_eq!(
        validate_backlog_doc(&text),
        Err(BacklogDocError::DuplicateId("B001".to_string()))
    );
}

#[test]
fn backlog_validator_rejects_complete_status_without_evidence() {
    let text = VALID_BACKLOG_MINIMUM.replace("- Status: open", "- Status: complete");

    assert_eq!(
        validate_backlog_doc(&text),
        Err(BacklogDocError::UnsupportedCompleteStatus(
            "B001".to_string()
        ))
    );
}
