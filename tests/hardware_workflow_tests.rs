use std::fs;
use std::path::Path;

fn read_repo_file(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

#[test]
fn hardware_workflow_is_manual_and_not_part_of_normal_pr_ci() {
    let hardware = read_repo_file(".github/workflows/hardware.yml");
    let ci = read_repo_file(".github/workflows/ci.yml");

    for required in [
        "name: opt-in hardware checks",
        "workflow_dispatch:",
        "runner_label:",
        "default: \"aegishv-hardware-kvm\"",
        "run_live_tracefs:",
        "default: false",
        "run_snapshot:",
        "permissions:",
        "contents: read",
        "runs-on: ${{ inputs.runner_label }}",
    ] {
        assert!(
            hardware.contains(required),
            "hardware workflow is missing opt-in guard or input: {required}"
        );
    }

    for forbidden in ["pull_request:", "push:", "schedule:", "workflow_run:"] {
        assert!(
            !hardware.contains(forbidden),
            "hardware workflow must not run from normal CI events: {forbidden}"
        );
    }

    assert!(
        !ci.contains("hardware.yml")
            && !ci.contains("aegishv-hardware-kvm")
            && !ci.contains("live-tracefs-smoke.sh"),
        "normal PR CI must not depend on hardware workflow or live tracefs smoke"
    );
}

#[test]
fn hardware_workflow_documents_live_host_prerequisites_without_secrets() {
    let hardware = read_repo_file(".github/workflows/hardware.yml");
    let testing = read_repo_file("docs/TESTING.md");

    for required in [
        "Self-hosted Linux runner label with reviewed KVM/tracefs permissions",
        "Run live tracefs smoke; requires KVM activity and tracefs write permission",
        "test -e /dev/kvm",
        "test -d /sys/kernel/tracing",
        "test -d /sys/kernel/debug/tracing",
        "scripts/live-tracefs-smoke.sh --timeout 30",
    ] {
        assert!(
            hardware.contains(required),
            "hardware workflow is missing live-host prerequisite or check: {required}"
        );
    }

    for required in [
        "Opt-In Hardware Workflow",
        "`.github/workflows/hardware.yml`",
        "`workflow_dispatch` only",
        "not triggered by normal `pull_request` or `push` events",
        "Linux self-hosted runner",
        "KVM-capable host with `/dev/kvm` present",
        "mounted tracefs",
        "enough guest activity to produce a real `kvm_exit` line",
        "optional libvirt/QMP permissions",
        "live tracefs smoke is controlled by `run_live_tracefs` and remains off by default",
        "does not prove type-1 support",
        "full VMI",
        "hardware PMU sampling",
    ] {
        assert!(
            testing.contains(required),
            "docs/TESTING.md is missing hardware workflow guidance: {required}"
        );
    }

    for forbidden in [
        "secrets.",
        "PRIVATE KEY",
        "BEGIN RSA",
        "C:\\Users",
        "/Users/",
        "/home/runner/work/",
    ] {
        assert!(
            !hardware.contains(forbidden) && !testing.contains(forbidden),
            "hardware workflow/docs contain forbidden secret or local path text: {forbidden}"
        );
    }
}

#[test]
fn hardware_workflow_does_not_claim_unimplemented_coverage() {
    let hardware = read_repo_file(".github/workflows/hardware.yml");
    let testing = read_repo_file("docs/TESTING.md");
    let combined = format!("{hardware}\n{testing}");

    for forbidden in [
        "type-1 coverage exists",
        "VMI coverage exists",
        "EPT/NPT enforcement coverage exists",
        "syscall integrity coverage exists",
        "hardware PMU coverage exists",
        "libvirt coverage exists",
        "production hardware coverage",
        "complete hardware validation",
        "guaranteed hardware support",
        "private hardware required for PR",
        "normal PR CI requires live KVM",
        "normal PR CI requires root",
    ] {
        assert!(
            !combined.contains(forbidden),
            "hardware workflow/docs contain fake or unsafe hardware claim: {forbidden}"
        );
    }
}
