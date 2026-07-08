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
fn workspace_and_lockfile_include_minimal_type1_kernel_crate() {
    let cargo = read_repo_file("Cargo.toml");
    let lock = read_repo_file("Cargo.lock");
    let manifest = read_repo_file("crates/aegishv-type1-kernel/Cargo.toml");

    assert!(cargo.contains("crates/aegishv-type1-kernel"));
    assert!(lock.contains("name = \"aegishv-type1-kernel\""));
    assert_contains_all(
        &manifest,
        &[
            "name = \"aegishv-type1-kernel\"",
            "minimal no_std AegisHV type-1 kernel entry artifact",
            "aegishv-type1-boot",
        ],
    );
}

#[test]
fn kernel_entry_records_serial_marker_and_halt_path() {
    let lib = read_repo_file("crates/aegishv-type1-kernel/src/lib.rs");
    let main = read_repo_file("crates/aegishv-type1-kernel/src/main.rs");
    let layout = read_repo_file("crates/aegishv-type1-boot/src/layout.rs");

    assert_contains_all(
        &lib,
        &[
            "SERIAL_READY_MARKER",
            "aegishv:type1:halt",
            "SERIAL_PANIC_MARKER",
            "marker_line",
        ],
    );
    assert_contains_all(
        &main,
        &[
            "global_asm!",
            "options(att_syntax)",
            "aegishv_type1_rust_entry",
            "serial_init",
            "serial_write_byte",
            "halt_loop",
        ],
    );
    assert!(layout.contains("0xffff_ffff_8020_0000"));
}

#[test]
fn kernel_build_script_and_ci_keep_boot_evidence_boundary() {
    let script = read_repo_file("scripts/build-type1-kernel.sh");
    let ci = read_repo_file(".github/workflows/ci.yml");
    let testing = read_repo_file("docs/TESTING.md");

    assert_contains_all(
        &script,
        &[
            "x86_64-unknown-none",
            "cargo rustc",
            "--bin aegishv-type1-kernel",
            "-C panic=abort",
            "-C strip=none",
            "-C link-arg=-T",
            "inspect-type1-kernel.sh",
            "bootable_image=false",
            "qemu_evidence=false",
            "not a bootable ISO",
        ],
    );
    assert_contains_all(
        &ci,
        &[
            "targets: x86_64-unknown-none",
            "cargo clippy --locked --workspace",
            "bash scripts/build-type1-kernel.sh",
        ],
    );
    assert!(testing.contains("scripts/build-type1-kernel.sh"));
    assert!(testing.contains("not a bootable ISO"));
}
