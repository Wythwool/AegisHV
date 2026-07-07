use std::fmt;
use std::fs;
use std::path::Path;

struct TemplateSpec {
    file: &'static str,
    required: &'static [&'static str],
}

const ISSUE_TEMPLATES: &[TemplateSpec] = &[
    TemplateSpec {
        file: ".github/ISSUE_TEMPLATE/bug_report.md",
        required: &[
            "Bug report",
            "Reproduction",
            "Observed Result",
            "Expected Result",
            "Safety Boundary",
            "Tests Run",
        ],
    },
    TemplateSpec {
        file: ".github/ISSUE_TEMPLATE/feature_request.md",
        required: &[
            "Feature request",
            "Scope",
            "Non-Goals",
            "Acceptance Criteria",
            "Schema Impact",
            "Security Impact",
            "Unsupported Claims Check",
        ],
    },
    TemplateSpec {
        file: ".github/ISSUE_TEMPLATE/backend_task.md",
        required: &[
            "Backend task",
            "Current State",
            "Acceptance Criteria",
            "Production Gate",
            "unsupported backend",
            "typed error",
        ],
    },
    TemplateSpec {
        file: ".github/ISSUE_TEMPLATE/detector_task.md",
        required: &[
            "Detector task",
            "Data Source",
            "Acceptance Criteria",
            "False-Positive Impact",
            "False-Negative Impact",
            "Schema Impact",
        ],
    },
    TemplateSpec {
        file: ".github/ISSUE_TEMPLATE/release_task.md",
        required: &[
            "Release task",
            "Required Checks",
            "cargo fmt --all -- --check",
            "cargo clippy --locked --all-targets --all-features -- -D warnings",
            "cargo test --locked --all --all-features",
            "./scripts/smoke-replay.sh",
            "Production Claim Check",
        ],
    },
];

const PR_TEMPLATE: TemplateSpec = TemplateSpec {
    file: ".github/pull_request_template.md",
    required: &[
        "Tests Run",
        "Schema Impact",
        "Security Impact",
        "Docs Impact",
        "False-positive impact",
        "False-negative impact",
        "Production Claim Check",
    ],
};

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

const VALID_ISSUE_TEMPLATE: &str = r#"---
name: Example
about: Example issue.
title: "example: "
labels: example
---

## Required

Tests Run
"#;

#[derive(Debug, PartialEq, Eq)]
enum TemplateError {
    MissingFile(&'static str),
    EmptyFile(&'static str),
    MalformedFrontMatter(&'static str),
    MissingRequiredText {
        file: &'static str,
        text: &'static str,
    },
    ForbiddenPhrase {
        file: &'static str,
        phrase: &'static str,
    },
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TemplateError::MissingFile(file) => write!(f, "{file} is missing"),
            TemplateError::EmptyFile(file) => write!(f, "{file} is empty"),
            TemplateError::MalformedFrontMatter(file) => {
                write!(f, "{file} is missing GitHub template front matter")
            }
            TemplateError::MissingRequiredText { file, text } => {
                write!(f, "{file} is missing required text: {text}")
            }
            TemplateError::ForbiddenPhrase { file, phrase } => {
                write!(f, "{file} contains forbidden phrase: {phrase}")
            }
        }
    }
}

fn read_required_file(root: &Path, rel: &'static str) -> Result<String, TemplateError> {
    fs::read_to_string(root.join(rel)).map_err(|_| TemplateError::MissingFile(rel))
}

fn validate_issue_template(file: &'static str, text: &str) -> Result<(), TemplateError> {
    if text.trim().is_empty() {
        return Err(TemplateError::EmptyFile(file));
    }

    if !has_front_matter(text) {
        return Err(TemplateError::MalformedFrontMatter(file));
    }

    for needle in ["name:", "about:", "title:", "labels:"] {
        if !text.contains(needle) {
            return Err(TemplateError::MalformedFrontMatter(file));
        }
    }

    reject_forbidden_phrases(file, text)
}

fn validate_required_text(
    file: &'static str,
    text: &str,
    required: &'static [&'static str],
) -> Result<(), TemplateError> {
    for &needle in required {
        if !text.contains(needle) {
            return Err(TemplateError::MissingRequiredText { file, text: needle });
        }
    }

    Ok(())
}

fn reject_forbidden_phrases(file: &'static str, text: &str) -> Result<(), TemplateError> {
    let lower = text.to_ascii_lowercase();
    for &phrase in FORBIDDEN_PHRASES {
        if lower.contains(&phrase.to_ascii_lowercase()) {
            return Err(TemplateError::ForbiddenPhrase { file, phrase });
        }
    }

    Ok(())
}

fn has_front_matter(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("---\n") else {
        return false;
    };
    rest.contains("\n---\n")
}

#[test]
fn issue_templates_and_pr_checklist_exist_with_required_contracts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    for spec in ISSUE_TEMPLATES {
        let text = read_required_file(root, spec.file).expect("issue template must exist");
        validate_issue_template(spec.file, &text)
            .expect("issue template front matter must be valid");
        validate_required_text(spec.file, &text, spec.required)
            .expect("issue template must keep required prompts");
    }

    let pr_text = read_required_file(root, PR_TEMPLATE.file).expect("PR template must exist");
    reject_forbidden_phrases(PR_TEMPLATE.file, &pr_text)
        .expect("PR template must avoid banned wording");
    validate_required_text(PR_TEMPLATE.file, &pr_text, PR_TEMPLATE.required)
        .expect("PR template must keep required checklist items");
}

#[test]
fn issue_template_validator_accepts_front_matter() {
    assert_eq!(
        validate_issue_template("example.md", VALID_ISSUE_TEMPLATE),
        Ok(())
    );
}

#[test]
fn issue_template_validator_rejects_missing_front_matter() {
    assert_eq!(
        validate_issue_template("bad.md", "name: Bad\n"),
        Err(TemplateError::MalformedFrontMatter("bad.md"))
    );
}

#[test]
fn issue_template_validator_rejects_empty_template() {
    assert_eq!(
        validate_issue_template("empty.md", "\n "),
        Err(TemplateError::EmptyFile("empty.md"))
    );
}

#[test]
fn template_contract_rejects_missing_required_pr_checklist_item() {
    assert_eq!(
        validate_required_text(
            "pull_request_template.md",
            "Tests Run\n",
            &["Schema Impact"]
        ),
        Err(TemplateError::MissingRequiredText {
            file: "pull_request_template.md",
            text: "Schema Impact",
        })
    );
}

#[test]
fn template_contract_rejects_forbidden_release_claim() {
    assert_eq!(
        reject_forbidden_phrases("bad.md", RELEASE_CLAIM),
        Err(TemplateError::ForbiddenPhrase {
            file: "bad.md",
            phrase: RELEASE_CLAIM,
        })
    );
}
