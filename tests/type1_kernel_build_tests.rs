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
            "aegishv-arch-x86",
            "aegishv-hypervisor-core",
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
            "SERIAL_RUNTIME_BACKEND_NONE_MARKER",
            "aegishv:type1:backend-none",
            "SERIAL_PANIC_MARKER",
            "SERIAL_LIMINE_MISSING_MARKER",
            "SERIAL_LIMINE_MEMMAP_EMPTY_MARKER",
            "SERIAL_LIMINE_MEMMAP_ENTRIES_MARKER",
            "SERIAL_LIMINE_EXECUTABLE_PHYSICAL_MARKER",
            "SERIAL_LIMINE_EXECUTABLE_VIRTUAL_MARKER",
            "LIMINE_BASE_REVISION",
            "LIMINE_MEMMAP_REQUEST_ID",
            "LIMINE_EXECUTABLE_ADDRESS_REQUEST_ID",
            "LimineRequest",
            "LimineMinimalHandoff",
            "Type1CpuSnapshot",
            "Type1CapabilityReport",
            "type1_capabilities_from_snapshot",
            "Type1ControlSnapshot",
            "Type1RuntimePreflight",
            "Type1RuntimeEnablePlan",
            "plan_type1_runtime_preflight",
            "plan_type1_runtime_enable",
            "CPUID_SVM_FEATURE_LEAF",
            "IA32_FEATURE_CONTROL_MSR",
            "IA32_VMX_CR0_FIXED0_MSR",
            "IA32_VMX_CR4_FIXED1_MSR",
            "IA32_EFER_MSR",
            "TYPE1_CR4_VMXE",
            "SERIAL_RUNTIME_PREFLIGHT_OK_MARKER",
            "SERIAL_RUNTIME_PREFLIGHT_ERROR_MARKER",
            "SERIAL_RUNTIME_ENABLE_OK_MARKER",
            "SERIAL_RUNTIME_ENABLE_ERROR_MARKER",
            "Type1RuntimePlan",
            "build_vmx_runtime",
            "build_svm_runtime",
            "serial_marker",
            "marker_line",
        ],
    );
    assert_contains_all(
        &main,
        &[
            "global_asm!",
            "options(att_syntax)",
            ".limine_requests_start",
            ".limine_requests",
            ".limine_requests_end",
            "aegishv_type1_rust_entry",
            "read_limine_minimal_handoff",
            "runtime_markers",
            "read_type1_cpu_snapshot",
            "read_type1_control_snapshot",
            "read_cr0",
            "read_cr4",
            "write_cr0",
            "write_cr4",
            "write_msr",
            "apply_type1_enable_plan",
            "__cpuid_count",
            "read_msr",
            "plan_type1_runtime",
            "plan_type1_runtime_preflight",
            "plan_type1_runtime_enable",
            "type1_capabilities_from_snapshot",
            "limine_minimal_handoff_status",
            "LIMINE_RESPONSE_REVISION_OFFSET",
            "LIMINE_HHDM_OFFSET_OFFSET",
            "LIMINE_MEMMAP_ENTRY_COUNT_OFFSET",
            "LIMINE_MEMMAP_ENTRIES_OFFSET",
            "LIMINE_EXECUTABLE_PHYSICAL_BASE_OFFSET",
            "LIMINE_EXECUTABLE_VIRTUAL_BASE_OFFSET",
            "read_limine_response_u64",
            "read_volatile",
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
    let inspect = read_repo_file("scripts/inspect-type1-kernel.sh");
    let ci = read_repo_file(".github/workflows/ci.yml");
    let testing = read_repo_file("docs/TESTING.md");

    assert_contains_all(
        &script,
        &[
            "x86_64-unknown-none",
            "cargo rustc",
            "--bin aegishv-type1-kernel",
            "-C panic=abort",
            "-C relocation-model=static",
            "-C code-model=kernel",
            "-C strip=none",
            "-C link-arg=-T",
            "inspect-type1-kernel.sh",
            "AEGISHV_TYPE1_EXPECTED_PHYSICAL_BASE",
            "0x00200000",
            "expected_kernel_physical_base=",
            "AEGISHV_TYPE1_EXPECTED_VIRTUAL_BASE",
            "0xFFFFFFFF80200000",
            "expected_kernel_virtual_base=",
            "relocation_model=static",
            "code_model=kernel",
            "runtime_backend_marker=aegishv:type1:backend-none",
            "runtime_backend_probe=cpuid-msr",
            "runtime_backend_markers=aegishv:type1:backend-none,aegishv:type1:backend-vmx,aegishv:type1:backend-svm",
            "runtime_preflight=checked",
            "runtime_preflight_markers=aegishv:type1:runtime-preflight-ok,aegishv:type1:runtime-preflight-error",
            "runtime_enable=controlled",
            "runtime_enable_markers=aegishv:type1:runtime-enable-ok,aegishv:type1:runtime-enable-error",
            "bootable_image=false",
            "qemu_evidence=false",
            "not a bootable ISO",
        ],
    );
    assert_contains_all(
        &inspect,
        &[
            "AEGISHV_TYPE1_EXPECTED_RUNTIME_BACKEND",
            "aegishv:type1:backend-none",
            "aegishv:type1:backend-vmx",
            "aegishv:type1:backend-svm",
            "aegishv:type1:runtime-plan-error",
            "aegishv:type1:runtime-preflight-ok",
            "aegishv:type1:runtime-preflight-error",
            "aegishv:type1:runtime-enable-ok",
            "aegishv:type1:runtime-enable-error",
            "runtime backend marker was not found",
            "runtime preflight marker was not found",
            "runtime enable marker was not found",
            "runtime_backend_probe=cpuid-msr",
            "runtime_backend_markers_present=true",
            "runtime_backend_marker_present=true",
            "runtime_preflight=checked",
            "runtime_preflight_markers_present=true",
            "runtime_enable=controlled",
            "runtime_enable_markers_present=true",
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
