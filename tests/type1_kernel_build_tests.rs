use std::fs;
use std::path::Path;
use std::process::Command;

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
    let linker = read_repo_file("boot/linker/x86_64-type1.ld");
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
            "aegishv_install_transition_idt",
            "ltr ax",
            "lidt [rip + __aegishv_host_idtr]",
            "aegishv_type1_host_exception",
        ],
    );
    assert_contains_all(
        &linker,
        &[
            "__aegishv_double_fault_guard_bottom",
            "__aegishv_nmi_guard_bottom",
            "__aegishv_machine_check_guard_bottom",
            "__aegishv_vmx_exit_guard_bottom",
            "__aegishv_vmx_exit_stack_top",
            "__aegishv_boot_stack_guard_bottom",
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
            "pub mod host_paging",
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
            "SERIAL_HOST_PAGING_OK_MARKER",
            "SERIAL_HOST_PAGING_ERROR_MARKER",
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
fn live_type1_path_keeps_one_early_physical_memory_owner() {
    let main = read_repo_file("crates/aegishv-type1-kernel/src/main.rs");
    let allocator_call = "allocate_type1_runtime_memory_with_reservations::<";

    assert_eq!(
        main.matches(allocator_call).count(),
        1,
        "the live kernel must construct exactly one early allocator"
    );
    assert_contains_all(
        &main,
        &[
            "struct PreparedRuntimeMemory",
            "linked_kernel_reservation",
            "inherited_x86_cr3_root_reservation",
            "&[kernel_reservation, active_cr3_root]",
            "runtime_memory.allocation.allocate_intel_toy_guest()",
            "current_cr3_root != runtime_memory.inherited_cr3_root",
        ],
    );

    let guest_path = main
        .split_once("unsafe fn run_type1_vmx_toy_guest")
        .expect("live VMX guest path")
        .1
        .split_once("fn vmx_guest_entry_error")
        .expect("end of live VMX guest path")
        .0;
    assert!(!guest_path.contains("copy_limine_memory_entries("));
    assert!(!guest_path.contains(allocator_call));
}

#[test]
fn final_vmx_path_switches_to_owned_paging_after_hhdm_materialization() {
    let main = read_repo_file("crates/aegishv-type1-kernel/src/main.rs");
    let guest_path = main
        .split_once("unsafe fn run_type1_vmx_toy_guest")
        .expect("live VMX guest path")
        .1
        .split_once("fn vmx_guest_entry_error")
        .expect("end of live VMX guest path")
        .0;

    let guest_materialization = guest_path
        .find("materialize_type1_toy_guest")
        .expect("guest HHDM materialization");
    let paging_prepare = guest_path
        .find("prepare_owned_host_page_tables")
        .expect("owned host page-table preparation");
    let pat_gate = guest_path
        .find("validate_owned_host_mappings")
        .expect("owned host PAT0 write-back gate");
    let paging_activate = guest_path
        .find("activate_owned_host_paging")
        .expect("owned host CR3 activation");
    let host_capture = guest_path
        .find("capture_vmx_host_state")
        .expect("VMCS host-state capture");
    let vmx_launch = guest_path.find("aegishv_vmx_launch").expect("VMX launch");

    assert!(guest_materialization < paging_prepare);
    assert!(paging_prepare < pat_gate);
    assert!(pat_gate < paging_activate);
    assert!(paging_activate < host_capture);
    assert!(host_capture < vmx_launch);
    let after_activation = &guest_path[paging_activate..];
    assert!(!after_activation.contains("HhdmPageWriter"));
    assert!(!after_activation.contains("physical_to_hhdm"));
    assert_contains_all(
        &main,
        &[
            "SERIAL_HOST_PAGING_OK_MARKER",
            "SERIAL_HOST_PAGING_ERROR_MARKER",
            "X86_CR0_WRITE_PROTECT",
            "X86_EFER_NO_EXECUTE_ENABLE",
            "X86_CR4_FIVE_LEVEL_PAGING",
            "validate_materialized_tables",
            "write_cr3(root)",
        ],
    );
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
            "runtime_vmx_guest=bounded-io-a-io-b-cpuid-rdmsr-pat-nm-x87-nm-simd-hlt",
            "aegishv:type1:guest-preempt-exit-ok",
            "aegishv:type1:guest-io-exit-ok",
            "aegishv:type1:guest-io-b-exit-ok",
            "aegishv:type1:guest-rdmsr-exit-ok",
            "aegishv:type1:guest-pat-state-ok",
            "aegishv:type1:guest-nm-x87-exit-ok",
            "aegishv:type1:guest-nm-simd-exit-ok",
            "runtime_host_fpu_simd=guarded-probes-no-host-state-use",
            "runtime_vmx_diagnostics=cpu-signature,timer-rate,timer-reload,timer-effective",
            "runtime_vmx_diagnostic_prefixes=aegishv:type1:vmx-cpu-signature=0x,aegishv:type1:vmx-timer-rate=0x,aegishv:type1:vmx-timer-reload=0x,aegishv:type1:vmx-timer-effective=0x",
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
            "aegishv:type1:guest-preempt-exit-ok",
            "aegishv:type1:guest-io-exit-ok",
            "aegishv:type1:guest-io-b-exit-ok",
            "aegishv:type1:guest-rdmsr-exit-ok",
            "aegishv:type1:guest-pat-state-ok",
            "aegishv:type1:guest-nm-x87-exit-ok",
            "aegishv:type1:guest-nm-simd-exit-ok",
            "aegishv:type1:guest-pat-state-error",
            "aegishv:type1:guest-nm-x87-exit-error",
            "aegishv:type1:guest-nm-simd-exit-error",
            "aegishv:type1:guest-timeout",
            "aegishv:type1:vmx-cpu-signature=0x",
            "aegishv:type1:vmx-timer-rate=0x",
            "aegishv:type1:vmx-timer-reload=0x",
            "aegishv:type1:vmx-timer-effective=0x",
            "VMX diagnostic prefix was not found",
            "vmx_diagnostic_prefixes_present=true",
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
            "load_segment_page_alignment=\"passed\"",
            "load_segment_permissions=\"passed\"",
            "PT_LOAD contains malformed layout fields",
            "expected exactly three PT_LOAD segments",
            "expected RX, R, and RW PT_LOAD permissions",
            "Offset:",
            "VirtualAddress:",
            "PhysicalAddress:",
            "Alignment:",
            "alignment_value < 4096",
            "(alignment_value & (alignment_value - 1)) != 0",
            "PT_LOAD fields are not 4K aligned",
            "not congruent modulo p_align",
            "static_elf_check=\"passed\"",
            "static kernel contains relocations",
            "symbol_table_check=\"passed\"",
            "diagnostic symbol was not retained",
            "llvm-objdump --disassemble --section=.text --no-show-raw-insn",
            "check-type1-host-text.sh",
            "host_fpu_simd_text_check=\"passed\"",
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

#[test]
fn host_text_gate_rejects_fpu_simd_and_extended_state_instructions() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/tmp")
        .join(format!("type1-host-text-{}", std::process::id()));
    let fixture = directory.join("disassembly.txt");
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create host-text fixture directory");

    fs::write(
        &fixture,
        "ffffffff80200000: 48 89 c3\tmovq %rax, %rbx\nffffffff80200003: e8 00 00 00 00\tcallq 0 <xmm0>\nffffffff80200008: f4\thlt\n",
    )
    .expect("write safe disassembly");
    let safe = Command::new("bash")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .arg("scripts/check-type1-host-text.sh")
        .arg(&fixture)
        .output()
        .expect("run host-text check");
    assert!(
        safe.status.success(),
        "safe disassembly was rejected: {}",
        String::from_utf8_lossy(&safe.stderr)
    );

    for (case, instruction) in [
        ("x87", "fnop"),
        ("XMM", "movdqa %xmm0, %xmm0"),
        ("MMX", "emms"),
        ("AVX", "vzeroupper"),
        ("AVX MXCSR", "vldmxcsr (%rax)"),
        ("AVX MXCSR store", "vstmxcsr (%rax)"),
        ("XSAVE", "xsave64 (%rax)"),
        ("XCR read", "xgetbv"),
        ("XCR", "xsetbv"),
        ("PKRU read", "rdpkru"),
        ("PKRU", "wrpkru"),
        ("MPX", "bndmov %bnd0, %bnd1"),
        ("AMX", "tilezero %tmm0"),
        ("AMX config", "ldtilecfg (%rax)"),
    ] {
        fs::write(&fixture, format!("ffffffff80200000: 90\t{instruction}\n"))
            .expect("write forbidden disassembly");
        let output = Command::new("bash")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .arg("scripts/check-type1-host-text.sh")
            .arg(&fixture)
            .output()
            .expect("run host-text refusal check");
        assert_eq!(output.status.code(), Some(70), "case: {case}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("FPU/SIMD state instruction found"),
            "unexpected stderr for {case}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fs::write(&fixture, "Disassembly of section .text:\n").expect("write empty disassembly");
    let empty = Command::new("bash")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .arg("scripts/check-type1-host-text.sh")
        .arg(&fixture)
        .output()
        .expect("run empty host-text refusal check");
    assert_eq!(empty.status.code(), Some(70));
    assert!(String::from_utf8_lossy(&empty.stderr).contains("contains no instructions"));

    fs::remove_dir_all(directory).expect("remove host-text fixture directory");
}

#[test]
fn qemu_evidence_budget_matches_the_vmx_runtime_budget() {
    let capabilities = read_repo_file("crates/aegishv-arch-x86/src/vmx/capabilities.rs");
    let evidence = read_repo_file("scripts/type1-qemu-evidence.sh");

    assert!(capabilities.contains("VMX_TOY_GUEST_BUDGET_TSC_TICKS: u64 = 1 << 24"));
    assert!(evidence.contains("vmx_timer_budget_limit=\"0x0000000001000000\""));
}
