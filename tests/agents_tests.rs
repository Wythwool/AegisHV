use std::fmt;
use std::fs;
use std::path::Path;

const REQUIRED_CONTRACTS: &[(&str, &str)] = &[
    ("small scope", "small scope"),
    ("tests required", "tests required"),
    ("no-fake-type1", "no-fake-type1"),
    ("schema migration discipline", "schema migration discipline"),
    ("unsafe-code comments", "unsafe code"),
    ("dependency policy", "dependency policy"),
    (
        "unsupported behavior refusal",
        "unsupported behavior must return typed unsupported errors",
    ),
    ("creator", "https://github.com/Wythwool"),
    ("organization", "https://github.com/Nullbit1"),
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
];

const VALID_AGENTS_DOC_MINIMUM: &str = r#"
small scope
tests required
no-fake-type1
schema migration discipline
unsafe code
dependency policy
unsupported behavior must return typed unsupported errors
https://github.com/Wythwool
https://github.com/Nullbit1
"#;

#[derive(Debug, PartialEq, Eq)]
enum AgentsDocError {
    MissingFile,
    EmptyDocument,
    MissingRequiredRule(&'static str),
    ForbiddenPhrase(&'static str),
}

impl fmt::Display for AgentsDocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentsDocError::MissingFile => write!(f, "AGENTS.md is missing at the repository root"),
            AgentsDocError::EmptyDocument => write!(f, "AGENTS.md is empty"),
            AgentsDocError::MissingRequiredRule(rule) => {
                write!(f, "AGENTS.md is missing required rule: {rule}")
            }
            AgentsDocError::ForbiddenPhrase(phrase) => {
                write!(f, "AGENTS.md contains forbidden phrase: {phrase}")
            }
        }
    }
}

fn load_agents_doc(root: &Path) -> Result<String, AgentsDocError> {
    fs::read_to_string(root.join("AGENTS.md")).map_err(|_| AgentsDocError::MissingFile)
}

fn validate_agents_doc(text: &str) -> Result<(), AgentsDocError> {
    if text.trim().is_empty() {
        return Err(AgentsDocError::EmptyDocument);
    }

    let lower = text.to_ascii_lowercase();
    for &(rule, needle) in REQUIRED_CONTRACTS {
        let found = if needle.starts_with("https://") {
            text.contains(needle)
        } else {
            lower.contains(needle)
        };
        if !found {
            return Err(AgentsDocError::MissingRequiredRule(rule));
        }
    }

    for &phrase in FORBIDDEN_PHRASES {
        if lower.contains(phrase) {
            return Err(AgentsDocError::ForbiddenPhrase(phrase));
        }
    }

    Ok(())
}

#[test]
fn agents_file_exists_and_contains_no_fake_type1_rule() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = load_agents_doc(root).expect("AGENTS.md must exist at the repository root");

    validate_agents_doc(&text).expect("AGENTS.md must keep the maintainer PR contract");
    assert!(text.contains("no-fake-type1"));
}

#[test]
fn agents_contract_validator_accepts_required_rules() {
    assert_eq!(validate_agents_doc(VALID_AGENTS_DOC_MINIMUM), Ok(()));
}

#[test]
fn agents_contract_validator_rejects_missing_no_fake_type1_rule() {
    let text = VALID_AGENTS_DOC_MINIMUM.replace("no-fake-type1", "backend honesty");

    assert_eq!(
        validate_agents_doc(&text),
        Err(AgentsDocError::MissingRequiredRule("no-fake-type1"))
    );
}

#[test]
fn agents_contract_validator_rejects_malformed_empty_document() {
    assert_eq!(
        validate_agents_doc(" \n\t "),
        Err(AgentsDocError::EmptyDocument)
    );
}

#[test]
fn agents_contract_validator_rejects_fake_production_claims() {
    let text = format!("{VALID_AGENTS_DOC_MINIMUM}\n{RELEASE_CLAIM}\n");

    assert_eq!(
        validate_agents_doc(&text),
        Err(AgentsDocError::ForbiddenPhrase(RELEASE_CLAIM))
    );
}
