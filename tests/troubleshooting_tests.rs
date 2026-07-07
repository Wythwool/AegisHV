use std::fmt;
use std::fs;
use std::path::Path;

const REQUIRED_TOPICS: &[(&str, &[&str])] = &[
    (
        "config parser errors",
        &["validate-config", "line-numbered parser error"],
    ),
    ("metrics bind failures", &["metrics listener", "bind"]),
    ("JSONL output failures", &["JSONL output", "write error"]),
    ("JSONL reopen failures", &["jsonl_reopen_failed"]),
    (
        "disk spool limits and failures",
        &["aegishv_spool_dropped_total", "max_bytes"],
    ),
    ("tracefs format diagnostics", &["tracefs_format_diagnostic"]),
    ("live tracefs smoke failures", &["live-tracefs-smoke.sh"]),
    ("QMP action refusal", &["QMP action audit", "refused=true"]),
    ("QMP action failure", &["timeout or retry exhaustion"]),
    (
        "stable-QMP identity mismatch",
        &["identity.require_stable_qmp_match=true"],
    ),
    ("dump path rejection", &["dump_guest_memory", "dump_root"]),
    ("health readiness degraded states", &["/healthz", "/readyz"]),
    ("shutdown behavior", &["SIGINT", "SIGTERM"]),
    ("reload behavior", &["SIGHUP", "startup-only"]),
    ("systemd failures", &["systemd", "journalctl"]),
    (
        "PMU limitations",
        &["PMU output", "not full PEBS/IBS/SPE sampling"],
    ),
];

const FORBIDDEN_CLAIMS: &[&str] = &[
    "provides type-1",
    "full VMI support",
    "EPT/NPT enforcement is implemented",
    "syscall-path integrity is implemented",
    "hardware PMU sampling is implemented",
    concat!("production", "-", "ready"),
];

#[derive(Debug, PartialEq, Eq)]
enum TroubleshootingDocError {
    MissingDoc,
    MissingTopic(&'static str),
    MissingTopicText {
        topic: &'static str,
        text: &'static str,
    },
    ForbiddenClaim(&'static str),
}

impl fmt::Display for TroubleshootingDocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TroubleshootingDocError::MissingDoc => write!(f, "docs/TROUBLESHOOTING.md is missing"),
            TroubleshootingDocError::MissingTopic(topic) => {
                write!(f, "docs/TROUBLESHOOTING.md is missing topic: {topic}")
            }
            TroubleshootingDocError::MissingTopicText { topic, text } => {
                write!(
                    f,
                    "docs/TROUBLESHOOTING.md topic {topic} is missing text: {text}"
                )
            }
            TroubleshootingDocError::ForbiddenClaim(claim) => {
                write!(
                    f,
                    "docs/TROUBLESHOOTING.md contains forbidden claim: {claim}"
                )
            }
        }
    }
}

fn load_troubleshooting_doc(root: &Path) -> Result<String, TroubleshootingDocError> {
    fs::read_to_string(root.join("docs/TROUBLESHOOTING.md"))
        .map_err(|_| TroubleshootingDocError::MissingDoc)
}

fn validate_troubleshooting_doc(text: &str) -> Result<(), TroubleshootingDocError> {
    for &(topic, needles) in REQUIRED_TOPICS {
        if !text.contains(topic) {
            return Err(TroubleshootingDocError::MissingTopic(topic));
        }
        for &needle in needles {
            if !text.contains(needle) {
                return Err(TroubleshootingDocError::MissingTopicText {
                    topic,
                    text: needle,
                });
            }
        }
    }

    let lower = text.to_ascii_lowercase();
    for &claim in FORBIDDEN_CLAIMS {
        if lower.contains(&claim.to_ascii_lowercase()) {
            return Err(TroubleshootingDocError::ForbiddenClaim(claim));
        }
    }

    Ok(())
}

#[test]
fn troubleshooting_doc_covers_real_operator_failure_modes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = load_troubleshooting_doc(root).expect("troubleshooting doc must exist");

    validate_troubleshooting_doc(&text)
        .expect("troubleshooting doc must cover concrete operator failures");
}

#[test]
fn troubleshooting_validator_rejects_missing_required_topic() {
    let mut text = String::from("# Troubleshooting\n");
    for &(topic, needles) in REQUIRED_TOPICS {
        if topic == "config parser errors" {
            continue;
        }
        text.push_str(topic);
        text.push('\n');
        for &needle in needles {
            text.push_str(needle);
            text.push('\n');
        }
    }

    assert_eq!(
        validate_troubleshooting_doc(&text),
        Err(TroubleshootingDocError::MissingTopic(
            "config parser errors"
        ))
    );
}

#[test]
fn troubleshooting_validator_rejects_unsupported_capability_claim() {
    let mut text = String::from("# Troubleshooting\n");
    for &(topic, needles) in REQUIRED_TOPICS {
        text.push_str(topic);
        text.push('\n');
        for &needle in needles {
            text.push_str(needle);
            text.push('\n');
        }
    }
    text.push_str("full VMI support\n");

    assert_eq!(
        validate_troubleshooting_doc(&text),
        Err(TroubleshootingDocError::ForbiddenClaim("full VMI support"))
    );
}
