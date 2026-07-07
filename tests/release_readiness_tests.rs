use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn repo_file(rel: &str) -> String {
    fs::read_to_string(repo_root().join(rel)).unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

fn assert_contains_all(text: &str, required: &[&str]) {
    for item in required {
        assert!(text.contains(item), "missing required text: {item}");
    }
}

#[test]
fn release_gate_docs_cover_host_vmi_and_type1_boundaries() {
    let checklist = repo_file("RELEASE_CHECKLIST.md");
    let security = repo_file("docs/SECURITY_REVIEW_CHECKLIST.md");
    let gaps = repo_file("docs/DEPLOYMENT_GAP_REVIEW.md");
    let host = repo_file("docs/HOST_SENSOR_RELEASE_PLAN.md");
    let vmi = repo_file("docs/VMI_ALPHA_GATE.md");
    let gate = repo_file("docs/TYPE1_READINESS_GATE.md");
    let milestone = repo_file("docs/TYPE1_LAB_MILESTONE.md");
    let status = repo_file("docs/STATUS.md");
    let changelog = repo_file("CHANGELOG.md");

    assert_contains_all(
        &checklist,
        &[
            "cargo clippy --locked --all-targets --all-features -- -D warnings",
            "scripts/check-doc-links.sh",
            "unsupported backend claim",
            "benchmark number without raw output",
        ],
    );
    assert_contains_all(
        &security,
        &[
            "QMP actions require stable VM identity",
            "Dump paths stay inside the configured dump root",
            "must remain documented as not implemented",
        ],
    );
    assert_contains_all(
        &gaps,
        &[
            "Live guest memory reads are not implemented",
            "Bootable type-1 image is not present",
            "VMI alpha and type-1 lab milestones must remain separate",
        ],
    );
    assert_contains_all(
        &host,
        &[
            "host-side KVM telemetry sensor only",
            "docs/HARDWARE_TEST_MATRIX.md",
            "unsupported backend paths",
        ],
    );
    assert_contains_all(
        &vmi,
        &[
            "must not be described as full VMI",
            "Offline x86_64 four-level and LA57 translation fixtures pass",
            "Live guest reads and real OS profile extraction are not implemented",
        ],
    );
    assert_contains_all(
        &gate,
        &[
            "does not pass this gate",
            "no bootable type-1 runtime",
            "must not say that the current binary is a type-1 hypervisor",
        ],
    );
    assert_contains_all(
        &milestone,
        &[
            "not a bootable hypervisor",
            "boot image path and checksum",
            "Do not describe the current host-side binary as a type-1 hypervisor",
        ],
    );
    assert!(status.contains("Release gate documents"));
    assert!(changelog.contains("## Unreleased"));
}

#[test]
fn doc_link_checker_runs_when_bash_is_available() {
    let output = match Command::new("bash")
        .current_dir(repo_root())
        .arg("scripts/check-doc-links.sh")
        .output()
    {
        Ok(output) => output,
        Err(err) if err.kind() == ErrorKind::NotFound => return,
        Err(err) => panic!("run bash: {err}"),
    };

    assert!(
        output.status.success(),
        "doc link check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("checked markdown links"));
}
