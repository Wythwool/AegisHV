use std::fmt;
use std::fs;
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
enum LiveTracefsSmokeError {
    MissingScript,
    EmptyScript,
    MissingRequiredText(&'static str),
    FakePass,
}

impl fmt::Display for LiveTracefsSmokeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LiveTracefsSmokeError::MissingScript => {
                write!(f, "scripts/live-tracefs-smoke.sh is missing")
            }
            LiveTracefsSmokeError::EmptyScript => {
                write!(f, "scripts/live-tracefs-smoke.sh is empty")
            }
            LiveTracefsSmokeError::MissingRequiredText(text) => {
                write!(
                    f,
                    "live tracefs smoke script is missing required text: {text}"
                )
            }
            LiveTracefsSmokeError::FakePass => {
                write!(
                    f,
                    "live tracefs smoke script can claim success without live data"
                )
            }
        }
    }
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn load_script(root: &Path) -> Result<String, LiveTracefsSmokeError> {
    fs::read_to_string(root.join("scripts/live-tracefs-smoke.sh"))
        .map_err(|_| LiveTracefsSmokeError::MissingScript)
}

fn validate_live_tracefs_smoke_script(text: &str) -> Result<(), LiveTracefsSmokeError> {
    if text.trim().is_empty() {
        return Err(LiveTracefsSmokeError::EmptyScript);
    }

    for required in [
        "set -euo pipefail",
        "requires Linux tracefs",
        "events/kvm/kvm_exit",
        "trace_pipe",
        "trace_marker",
        "tracing_on",
        "restore_tracefs_state",
        "trap restore_tracefs_state EXIT INT TERM",
        "no live kvm_exit tracefs data observed",
        "observed kvm_exit data after trace marker",
    ] {
        if !text.contains(required) {
            return Err(LiveTracefsSmokeError::MissingRequiredText(required));
        }
    }

    let pass_index = text
        .find("observed kvm_exit data after trace marker")
        .ok_or(LiveTracefsSmokeError::MissingRequiredText(
            "observed kvm_exit data after trace marker",
        ))?;
    let sample_check_index =
        text.find("[ -n \"$sample\" ]")
            .ok_or(LiveTracefsSmokeError::MissingRequiredText(
                "[ -n \"$sample\" ]",
            ))?;

    if pass_index < sample_check_index {
        return Err(LiveTracefsSmokeError::FakePass);
    }

    Ok(())
}

#[test]
fn live_tracefs_smoke_script_has_explicit_live_data_contract() {
    let script = load_script(repo_root()).expect("live tracefs smoke script must exist");

    validate_live_tracefs_smoke_script(&script)
        .expect("live tracefs smoke script must keep explicit host checks");
}

#[test]
fn live_tracefs_smoke_validator_rejects_empty_script() {
    assert_eq!(
        validate_live_tracefs_smoke_script(" \n\t"),
        Err(LiveTracefsSmokeError::EmptyScript)
    );
}

#[test]
fn live_tracefs_smoke_validator_rejects_fake_pass_without_sample_check() {
    let fake = r#"#!/usr/bin/env bash
set -euo pipefail
requires Linux tracefs
events/kvm/kvm_exit
trace_pipe
trace_marker
tracing_on
restore_tracefs_state
trap restore_tracefs_state EXIT INT TERM
no live kvm_exit tracefs data observed
observed kvm_exit data after trace marker
"#;

    assert_eq!(
        validate_live_tracefs_smoke_script(fake),
        Err(LiveTracefsSmokeError::MissingRequiredText(
            "[ -n \"$sample\" ]"
        ))
    );
}
