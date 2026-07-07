use std::fmt;
use std::fs;
use std::path::Path;

const ADR_INDEX: &str = "docs/adr/README.md";
const ADR_0001: &str = "docs/adr/0001-record-architecture-decisions.md";

const REQUIRED_SECTIONS: &[&str] = &[
    "## Status",
    "## Context",
    "## Decision",
    "## Consequences",
    "## Test Impact",
];

const VALID_STATUSES: &[&str] = &["Proposed", "Accepted", "Superseded", "Rejected"];

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

const VALID_ADR_MINIMUM: &str = r#"
# ADR-9999: Example Decision

## Status

Accepted.

## Context

Context text.

## Decision

Decision text.

## Consequences

Consequence text.

## Test Impact

Test impact text.
"#;

#[derive(Debug, PartialEq, Eq)]
enum AdrDocError {
    MissingFile(&'static str),
    EmptyDocument(&'static str),
    MissingSection(&'static str),
    MissingStatus,
    InvalidStatus(String),
    MissingIndexEntry(&'static str),
    MissingArchitectureLink,
    ForbiddenPhrase(&'static str),
}

impl fmt::Display for AdrDocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdrDocError::MissingFile(file) => write!(f, "{file} is missing"),
            AdrDocError::EmptyDocument(file) => write!(f, "{file} is empty"),
            AdrDocError::MissingSection(section) => write!(f, "ADR is missing section: {section}"),
            AdrDocError::MissingStatus => write!(f, "ADR status section has no status line"),
            AdrDocError::InvalidStatus(status) => write!(f, "ADR has invalid status: {status}"),
            AdrDocError::MissingIndexEntry(entry) => {
                write!(f, "ADR index is missing entry: {entry}")
            }
            AdrDocError::MissingArchitectureLink => {
                write!(f, "docs/ARCHITECTURE.md does not link to the ADR index")
            }
            AdrDocError::ForbiddenPhrase(phrase) => {
                write!(f, "ADR text contains forbidden phrase: {phrase}")
            }
        }
    }
}

fn read_required_file(root: &Path, rel: &'static str) -> Result<String, AdrDocError> {
    fs::read_to_string(root.join(rel)).map_err(|_| AdrDocError::MissingFile(rel))
}

fn validate_adr_doc(file: &'static str, text: &str) -> Result<(), AdrDocError> {
    if text.trim().is_empty() {
        return Err(AdrDocError::EmptyDocument(file));
    }

    reject_forbidden_phrases(text)?;

    for &section in REQUIRED_SECTIONS {
        if !text.contains(section) {
            return Err(AdrDocError::MissingSection(section));
        }
    }

    let status = status_value(text).ok_or(AdrDocError::MissingStatus)?;
    if !VALID_STATUSES.contains(&status.as_str()) {
        return Err(AdrDocError::InvalidStatus(status));
    }

    Ok(())
}

fn validate_adr_index(text: &str) -> Result<(), AdrDocError> {
    if text.trim().is_empty() {
        return Err(AdrDocError::EmptyDocument(ADR_INDEX));
    }
    reject_forbidden_phrases(text)?;
    for entry in [
        "ADR-0001",
        "0001-record-architecture-decisions.md",
        "Accepted",
    ] {
        if !text.contains(entry) {
            return Err(AdrDocError::MissingIndexEntry(entry));
        }
    }
    Ok(())
}

fn validate_architecture_link(text: &str) -> Result<(), AdrDocError> {
    if text.contains("docs/adr/README.md") {
        Ok(())
    } else {
        Err(AdrDocError::MissingArchitectureLink)
    }
}

fn reject_forbidden_phrases(text: &str) -> Result<(), AdrDocError> {
    let lower = text.to_ascii_lowercase();
    for &phrase in FORBIDDEN_PHRASES {
        if lower.contains(&phrase.to_ascii_lowercase()) {
            return Err(AdrDocError::ForbiddenPhrase(phrase));
        }
    }
    Ok(())
}

fn status_value(text: &str) -> Option<String> {
    let mut in_status = false;
    for line in text.lines() {
        if line == "## Status" {
            in_status = true;
            continue;
        }
        if in_status && line.starts_with("## ") {
            return None;
        }
        if in_status {
            let trimmed = line.trim().trim_end_matches('.');
            if !trimmed.is_empty() {
                return trimmed
                    .split_whitespace()
                    .next()
                    .map(|value| value.to_string());
            }
        }
    }
    None
}

#[test]
fn adr_index_first_record_and_architecture_link_are_present() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let index = read_required_file(root, ADR_INDEX).expect("ADR index must exist");
    let adr = read_required_file(root, ADR_0001).expect("ADR-0001 must exist");
    let architecture =
        read_required_file(root, "docs/ARCHITECTURE.md").expect("architecture doc must exist");

    validate_adr_index(&index).expect("ADR index must list ADR-0001");
    validate_adr_doc(ADR_0001, &adr).expect("ADR-0001 must keep required sections");
    validate_architecture_link(&architecture).expect("architecture doc must link to ADR index");
}

#[test]
fn adr_validator_accepts_required_sections_and_status() {
    assert_eq!(validate_adr_doc("example.md", VALID_ADR_MINIMUM), Ok(()));
}

#[test]
fn adr_validator_rejects_missing_test_impact() {
    let text = VALID_ADR_MINIMUM.replace("## Test Impact\n\nTest impact text.\n", "");

    assert_eq!(
        validate_adr_doc("bad.md", &text),
        Err(AdrDocError::MissingSection("## Test Impact"))
    );
}

#[test]
fn adr_validator_rejects_empty_status() {
    let text = VALID_ADR_MINIMUM.replace("Accepted.\n", "");

    assert_eq!(
        validate_adr_doc("bad.md", &text),
        Err(AdrDocError::MissingStatus)
    );
}

#[test]
fn adr_validator_rejects_invalid_status() {
    let text = VALID_ADR_MINIMUM.replace("Accepted.", "Draft.");

    assert_eq!(
        validate_adr_doc("bad.md", &text),
        Err(AdrDocError::InvalidStatus("Draft".to_string()))
    );
}

#[test]
fn adr_validator_rejects_forbidden_release_claim() {
    let text = format!("{VALID_ADR_MINIMUM}\n{RELEASE_CLAIM}\n");

    assert_eq!(
        validate_adr_doc("bad.md", &text),
        Err(AdrDocError::ForbiddenPhrase(RELEASE_CLAIM))
    );
}
