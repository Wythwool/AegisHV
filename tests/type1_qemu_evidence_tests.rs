use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

struct FakeQemuLab {
    directory: PathBuf,
    image: PathBuf,
    qemu: PathBuf,
    serial_log: PathBuf,
}

impl FakeQemuLab {
    fn new() -> Self {
        let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        let directory = PathBuf::from(format!(
            "target/tmp/type1-qemu-evidence-{}-{id}",
            std::process::id()
        ));
        let image = directory.join("aegishv-type1.iso");
        let qemu = directory.join("fake-qemu.sh");
        let serial_log = directory.join("serial.log");

        let _ = fs::remove_dir_all(repo_root().join(&directory));
        fs::create_dir_all(repo_root().join(&directory)).expect("create fake QEMU lab");
        fs::write(repo_root().join(&image), b"test boot image").expect("write test boot image");
        fs::write(
            repo_root().join(&qemu),
            r#"#!/usr/bin/env bash
set -euo pipefail

if [ "${1:-}" = "--version" ]; then
  echo "QEMU emulator version test"
  exit 0
fi

serial_log=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -serial)
      serial_log="${2#file:}"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

if [ -z "$serial_log" ]; then
  echo "fake qemu: missing serial file" >&2
  exit 2
fi

printf '%s' "${AEGISHV_FAKE_SERIAL:-}" > "$serial_log"
"#,
        )
        .expect("write fake QEMU executable");

        let chmod = Command::new("bash")
            .current_dir(repo_root())
            .args(["-c", "chmod +x \"$1\"", "chmod"])
            .arg(&qemu)
            .status()
            .expect("mark fake QEMU executable");
        assert!(chmod.success(), "chmod failed for {}", qemu.display());

        Self {
            directory,
            image,
            qemu,
            serial_log,
        }
    }

    fn smoke(&self, serial: &str, args: &[&str]) -> Output {
        self.smoke_with_timeout_command(serial, args, None)
    }

    fn smoke_with_timeout_command(
        &self,
        serial: &str,
        args: &[&str],
        timeout_command: Option<&str>,
    ) -> Output {
        let mut command = Command::new("bash");
        clear_lab_environment(&mut command);
        command
            .current_dir(repo_root())
            .arg("scripts/type1-qemu-smoke.sh")
            .args(args)
            .arg(&self.image)
            .env("AEGISHV_QEMU", &self.qemu)
            .env("AEGISHV_QEMU_SERIAL_LOG", &self.serial_log)
            .env("AEGISHV_FAKE_SERIAL", serial);
        if let Some(timeout_command) = timeout_command {
            command.env("AEGISHV_TIMEOUT", timeout_command);
        }
        command.output().expect("run type-1 QEMU smoke")
    }

    fn evidence(&self, serial: &str, manifest: &Path) -> Output {
        let mut command = Command::new("bash");
        clear_lab_environment(&mut command);
        command
            .current_dir(repo_root())
            .arg("scripts/type1-qemu-evidence.sh")
            .args(["--image"])
            .arg(&self.image)
            .args(["--manifest"])
            .arg(manifest)
            .args(["--serial-log"])
            .arg(&self.serial_log)
            .env("AEGISHV_QEMU", &self.qemu)
            .env("AEGISHV_FAKE_SERIAL", serial)
            .output()
            .expect("run type-1 QEMU evidence")
    }
}

impl Drop for FakeQemuLab {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(repo_root().join(&self.directory));
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn clear_lab_environment(command: &mut Command) {
    for name in [
        "AEGISHV_TYPE1_BOOT_IMAGE",
        "AEGISHV_TYPE1_EXPECTED_MARKERS",
        "AEGISHV_TYPE1_EXPECTED_SERIAL",
        "AEGISHV_QEMU_MACHINE",
        "AEGISHV_QEMU_CPU",
        "AEGISHV_QEMU_TIMEOUT_SECONDS",
        "AEGISHV_TIMEOUT",
        "AEGISHV_TYPE1_OUT",
        "AEGISHV_TYPE1_QEMU_MANIFEST",
        "AEGISHV_QEMU_MANIFEST",
        "AEGISHV_QEMU_SERIAL_LOG",
    ] {
        command.env_remove(name);
    }
}

fn default_vmx_markers() -> &'static str {
    "aegishv:type1:host-tables-ok\n\
aegishv:type1:backend-vmx\n\
aegishv:type1:vmxon-cycle-ok\n\
aegishv:type1:vmcs-load-ok\n\
aegishv:type1:vmx-cpu-signature=0x000906ed\n\
aegishv:type1:vmx-timer-rate=0x00000005\n\
aegishv:type1:vmx-timer-reload=0x00080000\n\
aegishv:type1:vmx-timer-effective=0x0000000001000000\n\
aegishv:type1:guest-config-ok\n\
aegishv:type1:guest-preempt-exit-ok\n\
aegishv:type1:guest-io-exit-ok\n\
aegishv:type1:guest-cpuid-exit-ok\n\
aegishv:type1:guest-hlt-exit-ok\n\
aegishv:type1:guest-run-ok\n"
}

#[test]
fn qemu_smoke_requires_the_default_vmx_markers_in_order() {
    let lab = FakeQemuLab::new();
    let success = lab.smoke(default_vmx_markers(), &[]);
    assert!(
        success.status.success(),
        "ordered VMX smoke failed: {}",
        String::from_utf8_lossy(&success.stderr)
    );

    let wrong_order = lab.smoke(
        "aegishv:type1:host-tables-ok\n\
aegishv:type1:backend-vmx\n\
aegishv:type1:vmxon-cycle-ok\n\
aegishv:type1:vmcs-load-ok\n\
aegishv:type1:guest-config-ok\n\
aegishv:type1:guest-io-exit-ok\n\
aegishv:type1:guest-preempt-exit-ok\n\
aegishv:type1:guest-cpuid-exit-ok\n\
aegishv:type1:guest-hlt-exit-ok\n\
aegishv:type1:guest-run-ok\n",
        &[],
    );
    assert_eq!(wrong_order.status.code(), Some(70));
    assert!(
        String::from_utf8_lossy(&wrong_order.stderr)
            .contains("expected serial marker was not observed in required order"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&wrong_order.stderr)
    );

    let missing_io = lab.smoke(
        &default_vmx_markers().replace("aegishv:type1:guest-io-exit-ok\n", ""),
        &[],
    );
    assert_eq!(missing_io.status.code(), Some(70));
    assert!(
        String::from_utf8_lossy(&missing_io.stderr)
            .contains("required order: aegishv:type1:guest-io-exit-ok"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&missing_io.stderr)
    );
}

#[test]
fn qemu_smoke_accepts_repeated_marker_arguments() {
    let lab = FakeQemuLab::new();
    let output = lab.smoke(
        default_vmx_markers(),
        &[
            "--expect-marker",
            "aegishv:type1:host-tables-ok",
            "--expect-marker",
            "aegishv:type1:backend-vmx",
            "--expect-marker",
            "aegishv:type1:vmxon-cycle-ok",
            "--expect-marker",
            "aegishv:type1:vmcs-load-ok",
            "--expect-marker",
            "aegishv:type1:guest-config-ok",
            "--expect-marker",
            "aegishv:type1:guest-preempt-exit-ok",
            "--expect-marker",
            "aegishv:type1:guest-io-exit-ok",
            "--expect-marker",
            "aegishv:type1:guest-cpuid-exit-ok",
            "--expect-marker",
            "aegishv:type1:guest-hlt-exit-ok",
            "--expect-marker",
            "aegishv:type1:guest-run-ok",
        ],
    );

    assert!(
        output.status.success(),
        "repeated marker smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn qemu_smoke_rejects_a_weak_custom_marker_contract() {
    let lab = FakeQemuLab::new();
    let output = lab.smoke(
        "aegishv:type1:handoff-ok\n",
        &["--expect-markers", "aegishv:type1:handoff-ok"],
    );

    assert_eq!(output.status.code(), Some(64));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("preemption, port-I/O"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn qemu_smoke_rejects_a_contract_without_containment_markers() {
    let lab = FakeQemuLab::new();
    let old_contract = "aegishv:type1:host-tables-ok,aegishv:type1:backend-vmx,aegishv:type1:vmxon-cycle-ok,aegishv:type1:vmcs-load-ok,aegishv:type1:guest-config-ok,aegishv:type1:guest-cpuid-exit-ok,aegishv:type1:guest-hlt-exit-ok,aegishv:type1:guest-run-ok";
    let output = lab.smoke(default_vmx_markers(), &["--expect-markers", old_contract]);

    assert_eq!(output.status.code(), Some(64));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("preemption, port-I/O"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn qemu_smoke_rejects_backend_none_even_if_vmx_markers_follow() {
    let lab = FakeQemuLab::new();
    let output = lab.smoke(
        &format!("aegishv:type1:backend-none\n{}", default_vmx_markers()),
        &[],
    );

    assert_eq!(output.status.code(), Some(70));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("forbidden serial marker was observed: aegishv:type1:backend-none"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn qemu_smoke_rejects_failure_marker_after_success_chain() {
    let lab = FakeQemuLab::new();
    let output = lab.smoke(
        &format!("{}aegishv:type1:guest-entry-error\n", default_vmx_markers()),
        &[],
    );

    assert_eq!(output.status.code(), Some(70));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("forbidden serial marker was observed: aegishv:type1:guest-entry-error"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn qemu_smoke_rejects_guest_timeout_after_success_chain() {
    let lab = FakeQemuLab::new();
    let output = lab.smoke(
        &format!("{}aegishv:type1:guest-timeout\n", default_vmx_markers()),
        &[],
    );

    assert_eq!(output.status.code(), Some(70));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("forbidden serial marker was observed: aegishv:type1:guest-timeout"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn qemu_smoke_fails_closed_without_a_timeout_command() {
    let lab = FakeQemuLab::new();
    let output = lab.smoke_with_timeout_command(
        default_vmx_markers(),
        &[],
        Some("aegishv-timeout-command-does-not-exist"),
    );

    assert_eq!(output.status.code(), Some(69));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("a compatible timeout command was not found"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn evidence_manifest_accepts_the_ordered_vmx_chain() {
    let lab = FakeQemuLab::new();
    let manifest = lab.directory.join("evidence.txt");
    let output = lab.evidence(default_vmx_markers(), &manifest);

    assert!(
        output.status.success(),
        "ordered VMX evidence failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest_text =
        fs::read_to_string(repo_root().join(manifest)).expect("read QEMU evidence manifest");
    for expected in [
        "qemu_machine=q35,accel=kvm",
        "qemu_cpu=host,+vmx",
        "qemu_boot_mode=iso",
        "qemu_command=",
        "expected_serial_marker_count=10",
        "expected_serial_markers=aegishv:type1:host-tables-ok,aegishv:type1:backend-vmx,aegishv:type1:vmxon-cycle-ok,aegishv:type1:vmcs-load-ok,aegishv:type1:guest-config-ok,aegishv:type1:guest-preempt-exit-ok,aegishv:type1:guest-io-exit-ok,aegishv:type1:guest-cpuid-exit-ok,aegishv:type1:guest-hlt-exit-ok,aegishv:type1:guest-run-ok",
        "serial_markers_present=true",
        "serial_markers_in_order=true",
        "vmx_cpu_signature_valid=true",
        "vmx_cpu_signature=0x000906ed",
        "vmx_timer_rate_valid=true",
        "vmx_timer_rate=0x00000005",
        "vmx_timer_reload_valid=true",
        "vmx_timer_reload=0x00080000",
        "vmx_timer_effective_valid=true",
        "vmx_timer_effective=0x0000000001000000",
        "vmx_timer_semantics_valid=true",
        "vmx_timer_budget_limit=0x0000000001000000",
        "vmx_diagnostics_valid=true",
        "forbidden_backend_none_observed=false",
        "forbidden_marker_observed=false",
        "qemu_smoke_exit_status=0",
        "qemu_evidence_exit_status=0",
        "qemu_evidence=true",
    ] {
        assert!(
            manifest_text.contains(expected),
            "manifest is missing: {expected}"
        );
    }
}

#[test]
fn evidence_manifest_rejects_a_missing_vmx_diagnostic() {
    let lab = FakeQemuLab::new();
    let manifest = lab.directory.join("evidence.txt");
    let serial = default_vmx_markers().replace("aegishv:type1:vmx-timer-reload=0x00080000\n", "");
    let output = lab.evidence(&serial, &manifest);

    assert_eq!(output.status.code(), Some(70));
    let manifest_text =
        fs::read_to_string(repo_root().join(manifest)).expect("read QEMU evidence manifest");
    for expected in [
        "serial_markers_in_order=true",
        "vmx_timer_reload_valid=false",
        "vmx_timer_reload=\n",
        "vmx_timer_semantics_valid=false",
        "vmx_diagnostics_valid=false",
        "qemu_smoke_exit_status=0",
        "qemu_evidence_exit_status=70",
        "qemu_evidence=false",
    ] {
        assert!(
            manifest_text.contains(expected),
            "manifest is missing: {expected}"
        );
    }
}

#[test]
fn evidence_manifest_rejects_a_malformed_vmx_diagnostic() {
    let lab = FakeQemuLab::new();
    let manifest = lab.directory.join("evidence.txt");
    let serial = default_vmx_markers().replace(
        "aegishv:type1:vmx-timer-effective=0x0000000001000000",
        "aegishv:type1:vmx-timer-effective=0x000000000100000z",
    );
    let output = lab.evidence(&serial, &manifest);

    assert_eq!(output.status.code(), Some(70));
    let manifest_text =
        fs::read_to_string(repo_root().join(manifest)).expect("read QEMU evidence manifest");
    for expected in [
        "vmx_timer_effective_valid=false",
        "vmx_timer_effective=\n",
        "vmx_timer_semantics_valid=false",
        "vmx_diagnostics_valid=false",
        "qemu_smoke_exit_status=0",
        "qemu_evidence_exit_status=70",
        "qemu_evidence=false",
    ] {
        assert!(
            manifest_text.contains(expected),
            "manifest is missing: {expected}"
        );
    }
}

#[test]
fn evidence_manifest_rejects_semantically_invalid_vmx_timer_diagnostics() {
    let cases = [
        (
            "rate above architectural range",
            default_vmx_markers().replace(
                "aegishv:type1:vmx-timer-rate=0x00000005",
                "aegishv:type1:vmx-timer-rate=0x00000020",
            ),
        ),
        (
            "reserved reload sentinel",
            default_vmx_markers()
                .replace(
                    "aegishv:type1:vmx-timer-reload=0x00080000",
                    "aegishv:type1:vmx-timer-reload=0x00000001",
                )
                .replace(
                    "aegishv:type1:vmx-timer-effective=0x0000000001000000",
                    "aegishv:type1:vmx-timer-effective=0x0000000000000020",
                ),
        ),
        (
            "effective value mismatch",
            default_vmx_markers().replace(
                "aegishv:type1:vmx-timer-effective=0x0000000001000000",
                "aegishv:type1:vmx-timer-effective=0x0000000000800000",
            ),
        ),
        (
            "effective value above hard budget",
            default_vmx_markers()
                .replace(
                    "aegishv:type1:vmx-timer-reload=0x00080000",
                    "aegishv:type1:vmx-timer-reload=0x00100000",
                )
                .replace(
                    "aegishv:type1:vmx-timer-effective=0x0000000001000000",
                    "aegishv:type1:vmx-timer-effective=0x0000000002000000",
                ),
        ),
    ];

    for (case, serial) in cases {
        let lab = FakeQemuLab::new();
        let manifest = lab.directory.join("evidence.txt");
        let output = lab.evidence(&serial, &manifest);

        assert_eq!(output.status.code(), Some(70), "case: {case}");
        let manifest_text =
            fs::read_to_string(repo_root().join(manifest)).expect("read QEMU evidence manifest");
        for expected in [
            "vmx_timer_rate_valid=true",
            "vmx_timer_reload_valid=true",
            "vmx_timer_effective_valid=true",
            "vmx_timer_semantics_valid=false",
            "vmx_diagnostics_valid=false",
            "qemu_smoke_exit_status=0",
            "qemu_evidence_exit_status=70",
            "qemu_evidence=false",
        ] {
            assert!(
                manifest_text.contains(expected),
                "case {case} manifest is missing: {expected}"
            );
        }
    }
}

#[test]
fn evidence_manifest_rejects_duplicate_vmx_diagnostics() {
    let lab = FakeQemuLab::new();
    let manifest = lab.directory.join("evidence.txt");
    let serial = format!(
        "{}aegishv:type1:vmx-cpu-signature=0x000906ed\n",
        default_vmx_markers()
    );
    let output = lab.evidence(&serial, &manifest);

    assert_eq!(output.status.code(), Some(70));
    let manifest_text =
        fs::read_to_string(repo_root().join(manifest)).expect("read QEMU evidence manifest");
    for expected in [
        "vmx_cpu_signature_valid=false",
        "vmx_cpu_signature=\n",
        "vmx_diagnostics_valid=false",
        "qemu_smoke_exit_status=0",
        "qemu_evidence_exit_status=70",
        "qemu_evidence=false",
    ] {
        assert!(
            manifest_text.contains(expected),
            "manifest is missing: {expected}"
        );
    }
}

#[test]
fn evidence_manifest_records_order_and_backend_none_refusal() {
    let lab = FakeQemuLab::new();
    let manifest = lab.directory.join("evidence.txt");
    let output = lab.evidence(
        &format!("aegishv:type1:backend-none\n{}", default_vmx_markers()),
        &manifest,
    );

    assert_eq!(output.status.code(), Some(70));
    let manifest_text =
        fs::read_to_string(repo_root().join(manifest)).expect("read QEMU evidence manifest");
    for expected in [
        "expected_serial_marker_count=10",
        "expected_serial_markers=aegishv:type1:host-tables-ok,aegishv:type1:backend-vmx,aegishv:type1:vmxon-cycle-ok,aegishv:type1:vmcs-load-ok,aegishv:type1:guest-config-ok,aegishv:type1:guest-preempt-exit-ok,aegishv:type1:guest-io-exit-ok,aegishv:type1:guest-cpuid-exit-ok,aegishv:type1:guest-hlt-exit-ok,aegishv:type1:guest-run-ok",
        "serial_markers_present=true",
        "serial_markers_in_order=true",
        "forbidden_backend_none_observed=true",
        "forbidden_marker_observed=true",
        "forbidden_marker=aegishv:type1:backend-none",
        "qemu_smoke_exit_status=70",
        "qemu_evidence=false",
    ] {
        assert!(
            manifest_text.contains(expected),
            "manifest is missing: {expected}"
        );
    }
}

#[test]
fn evidence_manifest_records_guest_timeout_refusal() {
    let lab = FakeQemuLab::new();
    let manifest = lab.directory.join("evidence.txt");
    let output = lab.evidence(
        &format!("{}aegishv:type1:guest-timeout\n", default_vmx_markers()),
        &manifest,
    );

    assert_eq!(output.status.code(), Some(70));
    let manifest_text =
        fs::read_to_string(repo_root().join(manifest)).expect("read QEMU evidence manifest");
    for expected in [
        "serial_markers_present=true",
        "serial_markers_in_order=true",
        "forbidden_marker_observed=true",
        "forbidden_marker=aegishv:type1:guest-timeout",
        "qemu_smoke_exit_status=70",
        "qemu_evidence=false",
    ] {
        assert!(
            manifest_text.contains(expected),
            "manifest is missing: {expected}"
        );
    }
}
