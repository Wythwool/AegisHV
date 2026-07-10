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
    let build_rs = read_repo_file("crates/aegishv-type1-kernel/build.rs");

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
    assert!(build_rs.contains("cargo:rerun-if-changed=../../boot/linker/x86_64-type1.ld"));
}

#[test]
fn type1_entry_installs_owned_host_tables_before_rust() {
    let entry = read_repo_file("boot/x86_64/entry.S");
    let tables = read_repo_file("boot/x86_64/host_tables.S");
    let build = read_repo_file("scripts/build-type1-kernel.sh");

    let transition = entry.find("call aegishv_install_transition_idt").unwrap();
    let bss_clear = entry.find("lea __aegishv_bss_start").unwrap();
    let install = entry.find("call aegishv_install_host_tables").unwrap();
    let rust_entry = entry.find("call aegishv_type1_rust_entry").unwrap();
    assert!(transition < bss_clear);
    assert!(install < rust_entry);
    assert_contains_all(
        &tables,
        &[
            "__aegishv_host_gdt",
            "__aegishv_host_tss",
            "__aegishv_host_idt",
            "__aegishv_double_fault_stack_top",
            "__aegishv_nmi_stack_top",
            "__aegishv_machine_check_stack_top",
            "__aegishv_vmx_exit_stack_top",
            "aegishv_install_transition_idt",
            "ltr ax",
            "lidt [rip + __aegishv_host_idtr]",
            "aegishv_type1_host_exception",
        ],
    );
    let vmx_entry = read_repo_file("boot/x86_64/vmx_entry.S");
    assert_contains_all(
        &vmx_entry,
        &[
            "lgdt [rip + __aegishv_host_gdtr]",
            "lidt [rip + __aegishv_host_idtr]",
        ],
    );
    assert!(build.contains("-C no-redzone=yes"));
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
            "aegishv:type1:handoff-ok",
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
            "Type1VmxBasic",
            "Type1RuntimeRegionMaterialization",
            "Type1RuntimeMemoryAllocation",
            "allocate_type1_runtime_memory",
            "TYPE1_MAX_MEMORY_MAP_ENTRIES",
            "Type1VmxonCyclePlan",
            "Type1VmxonCycleStatus",
            "Type1VmxonCycleError",
            "Type1VmcsLoadCyclePlan",
            "Type1VmcsLoadCycleStatus",
            "Type1VmcsLoadCycleError",
            "plan_type1_runtime_preflight",
            "plan_type1_runtime_enable",
            "plan_type1_runtime_regions",
            "plan_type1_vmxon_cycle",
            "run_type1_vmxon_cycle_with",
            "plan_type1_vmcs_load_cycle",
            "run_type1_vmcs_load_cycle_with",
            "CPUID_SVM_FEATURE_LEAF",
            "IA32_FEATURE_CONTROL_MSR",
            "IA32_VMX_BASIC_MSR",
            "IA32_VMX_CR0_FIXED0_MSR",
            "IA32_VMX_CR4_FIXED1_MSR",
            "IA32_EFER_MSR",
            "TYPE1_CR4_VMXE",
            "SERIAL_RUNTIME_PREFLIGHT_OK_MARKER",
            "SERIAL_RUNTIME_PREFLIGHT_ERROR_MARKER",
            "SERIAL_RUNTIME_ENABLE_OK_MARKER",
            "SERIAL_RUNTIME_ENABLE_ERROR_MARKER",
            "SERIAL_RUNTIME_REGIONS_OK_MARKER",
            "SERIAL_RUNTIME_REGIONS_ERROR_MARKER",
            "SERIAL_RUNTIME_VMXON_OK_MARKER",
            "SERIAL_RUNTIME_VMXON_ERROR_MARKER",
            "SERIAL_RUNTIME_VMXON_SKIPPED_MARKER",
            "SERIAL_RUNTIME_VMCS_LOAD_OK_MARKER",
            "SERIAL_RUNTIME_VMCS_LOAD_ERROR_MARKER",
            "SERIAL_RUNTIME_VMCS_LOAD_SKIPPED_MARKER",
            "SERIAL_VMX_INSTRUCTION_ERROR_PREFIX",
            "aegishv:type1:vm-instruction-error=0x",
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
            "copy_limine_memory_entries",
            "read_type1_cpu_snapshot",
            "read_type1_control_snapshot",
            "read_cr0",
            "read_cr4",
            "write_cr0",
            "write_cr4",
            "write_msr",
            "read_type1_vmx_basic",
            "apply_type1_enable_plan",
            "materialize_type1_runtime_regions",
            "run_type1_vmcs_load_cycle",
            "physical_to_hhdm",
            "write_runtime_page",
            "write_runtime_revision",
            "__cpuid_count",
            "read_msr",
            "plan_type1_runtime_with_memory",
            "allocate_type1_runtime_memory",
            "plan_type1_runtime_preflight",
            "plan_type1_runtime_enable",
            "plan_type1_runtime_regions",
            "run_type1_vmcs_load_cycle_with",
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
    let workspace = read_repo_file("Cargo.toml");
    let ci = read_repo_file(".github/workflows/ci.yml");
    let testing = read_repo_file("docs/TESTING.md");

    assert_contains_all(
        &script,
        &[
            "x86_64-unknown-none",
            "cargo rustc",
            "--bin aegishv-type1-kernel",
            "--profile type1",
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
            "profile=type1",
            "runtime_backend_marker=aegishv:type1:backend-none",
            "runtime_backend_probe=cpuid-msr",
            "runtime_backend_markers=aegishv:type1:backend-none,aegishv:type1:backend-vmx,aegishv:type1:backend-svm",
            "runtime_preflight=checked",
            "runtime_preflight_markers=aegishv:type1:runtime-preflight-ok,aegishv:type1:runtime-preflight-error",
            "runtime_enable=controlled",
            "runtime_enable_markers=aegishv:type1:runtime-enable-ok,aegishv:type1:runtime-enable-error",
            "runtime_regions=materialized",
            "runtime_region_markers=aegishv:type1:runtime-regions-ok,aegishv:type1:runtime-regions-error",
            "runtime_vmxon=smoke-cycle",
            "runtime_vmxon_markers=aegishv:type1:vmxon-cycle-ok,aegishv:type1:vmxon-cycle-error,aegishv:type1:vmxon-cycle-skipped",
            "runtime_vmcs_load=smoke-cycle",
            "runtime_vmcs_load_markers=aegishv:type1:vmcs-load-ok,aegishv:type1:vmcs-load-error,aegishv:type1:vmcs-load-skipped",
            "bootable_image=false",
            "qemu_evidence=false",
            "not a bootable ISO",
        ],
    );
    assert_contains_all(
        &workspace,
        &[
            "[profile.type1]",
            "inherits = \"release\"",
            "strip = \"none\"",
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
            "aegishv:type1:runtime-regions-ok",
            "aegishv:type1:runtime-regions-error",
            "aegishv:type1:vmxon-cycle-ok",
            "aegishv:type1:vmxon-cycle-error",
            "aegishv:type1:vmxon-cycle-skipped",
            "aegishv:type1:vmcs-load-ok",
            "aegishv:type1:vmcs-load-error",
            "aegishv:type1:vmcs-load-skipped",
            "runtime backend marker was not found",
            "runtime preflight marker was not found",
            "runtime enable marker was not found",
            "runtime region marker was not found",
            "VMXON cycle marker was not found",
            "VMCS load marker was not found",
            "runtime_backend_probe=cpuid-msr",
            "runtime_backend_markers_present=true",
            "runtime_backend_marker_present=true",
            "runtime_preflight=checked",
            "runtime_preflight_markers_present=true",
            "runtime_enable=controlled",
            "runtime_enable_markers_present=true",
            "runtime_regions=materialized",
            "runtime_region_markers_present=true",
            "runtime_vmxon=smoke-cycle",
            "runtime_vmxon_markers_present=true",
            "runtime_vmcs_load=smoke-cycle",
            "runtime_vmcs_load_markers_present=true",
            "load_segment_permissions=\"passed\"",
            "expected RX, R, and RW PT_LOAD permissions",
            "static_elf_check=\"passed\"",
            "static kernel contains relocations",
            "symbol_table_check=\"passed\"",
            "diagnostic symbol was not retained",
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
