use std::fs;
use std::path::Path;

fn repo_file(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

fn assert_contains_all(text: &str, required: &[&str]) {
    for item in required {
        assert!(text.contains(item), "missing required text: {item}");
    }
}

#[test]
fn workspace_includes_boot_handoff_crate_and_lockfile_entry() {
    let cargo = repo_file("Cargo.toml");
    let lock = repo_file("Cargo.lock");
    let crate_manifest = repo_file("crates/aegishv-type1-boot/Cargo.toml");
    let lib = repo_file("crates/aegishv-type1-boot/src/lib.rs");

    assert!(cargo.contains("crates/aegishv-type1-boot"));
    assert!(lock.contains("name = \"aegishv-type1-boot\""));
    assert_contains_all(
        &crate_manifest,
        &[
            "name = \"aegishv-type1-boot\"",
            "no_std AegisHV boot handoff",
            "aegishv-hypervisor-core",
        ],
    );
    assert_contains_all(
        &lib,
        &[
            "TYPE1_BOOT_ABI_VERSION",
            "TYPE1_BOOT_MAGIC",
            "pub mod handoff",
            "pub mod image",
            "pub mod limine",
            "validate_boot_image_plan",
            "#![deny(unsafe_code)]",
        ],
    );
}

#[test]
fn boot_artifacts_define_limine_linker_and_entry_boundaries() {
    let limine = repo_file("boot/limine/limine.conf");
    let linker = repo_file("boot/linker/x86_64-type1.ld");
    let entry = repo_file("boot/x86_64/entry.S");
    let readme = repo_file("boot/README.md");

    assert_contains_all(
        &limine,
        &[
            "PROTOCOL=limine",
            "KERNEL_PATH=boot:///aegishv-type1.elf",
            "CMDLINE=serial=on",
        ],
    );
    assert_contains_all(
        &linker,
        &[
            "ENTRY(aegishv_type1_start)",
            "KERNEL_PHYS_BASE = 0x00200000",
            "__aegishv_boot_stack_top",
            "KEEP(*(.text.entry))",
        ],
    );
    assert_contains_all(
        &entry,
        &[
            ".global aegishv_type1_start",
            "cld",
            "__aegishv_bss_start",
            "__aegishv_bss_end",
            "rep stosb",
            "lea __aegishv_boot_stack_top",
            "call aegishv_type1_rust_entry",
            ".Lhalt",
        ],
    );
    assert_contains_all(
        &readme,
        &[
            "planned type-1 boot artifacts",
            "not wired into a bootable image build yet",
            "not a bootable hypervisor image",
        ],
    );
}

#[test]
fn boot_skeleton_script_writes_manifest_without_claiming_image_output() {
    let script = repo_file("scripts/build-type1-skeleton.sh");
    let doc = repo_file("docs/TYPE1_BOOT_BOUNDARY.md");
    let status = repo_file("docs/STATUS.md");
    let testing = repo_file("docs/TESTING.md");

    assert_contains_all(
        &script,
        &[
            "cargo test --locked -p aegishv-type1-boot --all-features",
            "bootable_image=false",
            "runtime_backend=false",
            "aegishv-type1-build-plan.txt",
            "plan-type1-image.sh",
            "image_plan_manifest=",
            "not a bootable hypervisor image",
        ],
    );
    assert_contains_all(
        &doc,
        &[
            "planned type-1 boot boundary",
            "not a bootable hypervisor image",
            "Bootable type-1 ISO is not produced",
            "QEMU boot evidence is not present",
        ],
    );
    assert!(status.contains("Planned type-1 boot skeleton artifacts"));
    assert!(testing.contains("scripts/build-type1-skeleton.sh"));
}
