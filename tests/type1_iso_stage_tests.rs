use std::fs;
use std::path::Path;

fn read_repo_file(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

fn assert_contains_all(text: &str, required: &[&str]) {
    for item in required {
        assert!(text.contains(item), "missing required text: {item}");
    }
}

#[test]
fn kernel_inspection_script_checks_entry_and_serial_marker() {
    let script = read_repo_file("scripts/inspect-type1-kernel.sh");
    let build = read_repo_file("scripts/build-type1-kernel.sh");
    let testing = read_repo_file("docs/TESTING.md");

    assert_contains_all(
        &script,
        &[
            "llvm-readobj --file-headers",
            "expected_entry",
            "0xFFFFFFFF80200000",
            "grep -Fqa",
            "serial_marker_present=true",
            "not QEMU boot evidence",
        ],
    );
    assert!(build.contains("inspect_manifest="));
    assert!(testing.contains("scripts/inspect-type1-kernel.sh"));
}

#[test]
fn limine_iso_stage_script_copies_current_inputs_without_claiming_boot() {
    let script = read_repo_file("scripts/stage-type1-limine-iso.sh");
    let ci = read_repo_file(".github/workflows/ci.yml");
    let testing = read_repo_file("docs/TESTING.md");

    assert_contains_all(
        &script,
        &[
            "limine-iso-root",
            "aegishv-type1.elf",
            "boot/aegishv-type1.elf",
            "limine.conf",
            "boot/limine/limine.conf",
            "limine_available=",
            "xorriso_available=",
            "bootable_iso=false",
            "qemu_evidence=false",
            "not a bootable ISO",
        ],
    );
    assert!(ci.contains("bash scripts/stage-type1-limine-iso.sh"));
    assert!(testing.contains("scripts/stage-type1-limine-iso.sh"));
}

#[test]
fn limine_iso_build_script_requires_real_tools_and_keeps_qemu_separate() {
    let script = read_repo_file("scripts/build-type1-limine-iso.sh");
    let testing = read_repo_file("docs/TESTING.md");

    assert_contains_all(
        &script,
        &[
            "AEGISHV_LIMINE_DIR",
            "xorriso -as mkisofs",
            "limine bios-install",
            "limine-bios.sys",
            "limine-bios-cd.bin",
            "limine-uefi-cd.bin",
            "bootable_iso=true",
            "qemu_evidence=false",
            "not QEMU boot evidence",
        ],
    );
    assert!(testing.contains("scripts/build-type1-limine-iso.sh"));
}

#[test]
fn qemu_smoke_supports_iso_boot_media() {
    let script = read_repo_file("scripts/type1-qemu-smoke.sh");

    assert_contains_all(
        &script,
        &[
            "boot_mode=\"iso\"",
            "-cdrom \"$image\"",
            "-boot d",
            "-kernel \"$image\"",
            "expected serial marker was not observed",
        ],
    );
}
