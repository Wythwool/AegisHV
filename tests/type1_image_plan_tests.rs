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
    let missing_kernel = "target/tmp/aegishv-type1-missing.elf";
    let _ = fs::remove_file(repo_root().join(missing_kernel));

    let output = Command::new("bash")
        .current_dir(repo_root())
        .args([
            "scripts/plan-type1-image.sh",
            "--manifest",
            manifest.to_str().unwrap(),
            "--kernel-elf",
            missing_kernel,
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
            "kernel_elf=target/tmp/aegishv-type1-missing.elf",
            "kernel_elf_present=false",
            "output_image=target/type1/aegishv-type1.iso",
            "qemu_expected_serial=aegishv:type1:handoff-ok",
            "expected_kernel_physical_base=0x00200000",
            "expected_kernel_virtual_base=0xFFFFFFFF80200000",
            "not a boot evidence record",
        ],
    );
}

#[test]
fn image_plan_script_can_require_the_future_kernel_elf() {
    let missing_kernel = "target/tmp/aegishv-type1-required-missing.elf";
    let _ = fs::remove_file(repo_root().join(missing_kernel));

    let output = Command::new("bash")
        .current_dir(repo_root())
        .args([
            "scripts/plan-type1-image.sh",
            "--require-kernel",
            "--kernel-elf",
            missing_kernel,
        ])
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
fn qemu_smoke_requires_ordered_vmx_evidence_markers() {
    let script = read_repo_file("scripts/type1-qemu-smoke.sh");
    let testing = read_repo_file("docs/TESTING.md");

    assert_contains_all(
        &script,
        &[
            "AEGISHV_TYPE1_EXPECTED_MARKERS",
            "--expect-markers",
            "--expect-marker",
            "aegishv:type1:backend-vmx",
            "aegishv:type1:vmxon-cycle-ok",
            "aegishv:type1:vmcs-load-ok",
            "aegishv:type1:backend-none",
            "serial log was not written",
            "expected serial marker was not observed in required order",
            "exit 70",
        ],
    );
    assert!(testing.contains("AEGISHV_TYPE1_EXPECTED_MARKERS"));
    assert!(testing.contains("complete serial lines"));
    assert!(testing.contains("non-VMX/skipped path"));
}
