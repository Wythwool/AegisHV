use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_repo_file(rel: &str) -> String {
    fs::read_to_string(repo_root().join(rel)).unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

fn assert_contains_all(text: &str, required: &[&str]) {
    for item in required {
        assert!(text.contains(item), "missing required text: {item}");
    }
}

#[test]
fn image_plan_script_writes_review_manifest_without_boot_claims() {
    let manifest = Path::new("target/tmp/aegishv-type1-image-plan-test.txt");
    let manifest_abs = repo_root().join(manifest);
    let _ = fs::remove_file(&manifest_abs);

    let output = Command::new("bash")
        .current_dir(repo_root())
        .args([
            "scripts/plan-type1-image.sh",
            "--manifest",
            manifest.to_str().unwrap(),
        ])
        .output()
        .expect("run type1 image plan helper");

    assert!(
        output.status.success(),
        "type1 image plan helper failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let text = fs::read_to_string(&manifest_abs).expect("read generated image plan manifest");
    assert_contains_all(
        &text,
        &[
            "aegishv type-1 image plan",
            "bootable_image=false",
            "runtime_backend=false",
            "kernel_elf=target/type1/aegishv-type1.elf",
            "kernel_elf_present=false",
            "output_image=target/type1/aegishv-type1.iso",
            "qemu_expected_serial=aegishv:type1:halt",
            "not a boot evidence record",
        ],
    );
}

#[test]
fn image_plan_script_can_require_the_future_kernel_elf() {
    let output = Command::new("bash")
        .current_dir(repo_root())
        .args(["scripts/plan-type1-image.sh", "--require-kernel"])
        .output()
        .expect("run type1 image plan helper with required kernel");

    assert_eq!(output.status.code(), Some(66));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("kernel ELF does not exist"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn qemu_smoke_requires_serial_evidence_marker() {
    let script = read_repo_file("scripts/type1-qemu-smoke.sh");
    let testing = read_repo_file("docs/TESTING.md");

    assert_contains_all(
        &script,
        &[
            "AEGISHV_TYPE1_EXPECTED_SERIAL",
            "--expect-serial",
            "aegishv:type1:halt",
            "serial log was not written",
            "expected serial marker was not observed",
            "exit 70",
        ],
    );
    assert!(testing.contains("AEGISHV_TYPE1_EXPECTED_SERIAL"));
    assert!(testing.contains("serial marker"));
}
