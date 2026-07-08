use std::fs;
use std::path::Path;

fn read_repo_file(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

#[test]
fn workspace_lists_no_std_type1_crates() {
    let cargo = read_repo_file("Cargo.toml");

    for member in [
        "crates/aegishv-hypervisor-core",
        "crates/aegishv-event-abi",
        "crates/aegishv-arch-x86",
        "crates/aegishv-arch-arm64",
        "crates/aegishv-devices",
        "crates/aegishv-type1-kernel",
    ] {
        assert!(cargo.contains(member), "workspace is missing {member}");
    }
}

#[test]
fn device_isolation_docs_keep_runtime_scope_explicit() {
    let smmu = read_repo_file("docs/adr/0006-arm-smmu-strategy.md");
    let location = read_repo_file("docs/adr/0007-device-model-location.md");
    let network = read_repo_file("docs/NETWORK_ISOLATION.md");
    let testing = read_repo_file("docs/TESTING.md");
    let status = read_repo_file("docs/STATUS.md");
    let index = read_repo_file("docs/adr/README.md");

    for required in [
        "Stream ID",
        "fail closed",
        "does not program SMMU hardware",
        "protected guest memory is not inspected",
    ] {
        assert!(smmu.contains(required), "SMMU ADR is missing: {required}");
    }

    for required in [
        "aegishv-devices",
        "not a live service VM",
        "Read-only block devices reject writes",
        "live MMIO exits",
    ] {
        assert!(
            location.contains(required),
            "device location ADR is missing: {required}"
        );
    }

    for required in [
        "Bridge And Tap Limits",
        "SR-IOV And Passthrough Limits",
        "assignment must fail closed",
        "does not program VT-d, AMD-Vi, or SMMU hardware",
        "Do not describe the current tree as having live network isolation",
    ] {
        assert!(
            network.contains(required),
            "network isolation doc is missing: {required}"
        );
    }

    assert!(testing.contains("cargo test --locked -p aegishv-devices --all-features"));
    assert!(testing.contains("They do not execute MMIO exits"));
    assert!(status.contains("Live device assignment"));
    assert!(index.contains("ADR-0006"));
    assert!(index.contains("ADR-0007"));
}

#[test]
fn boot_strategy_adr_records_first_boot_path_without_claiming_runtime_support() {
    let adr = read_repo_file("docs/adr/0002-type1-boot-strategy.md");
    let index = read_repo_file("docs/adr/README.md");

    for required in [
        "UEFI application",
        "Limine",
        "Multiboot2",
        "Custom loader",
        "Use Limine as the first boot protocol",
        "does not ship a bootable hypervisor image",
        "does not make the current binary a type-1 hypervisor",
    ] {
        assert!(
            adr.contains(required),
            "boot strategy ADR is missing: {required}"
        );
    }
    assert!(index.contains("ADR-0002"));
}

#[test]
fn ap_startup_adr_matches_validator_scope() {
    let adr = read_repo_file("docs/adr/0003-x86-ap-startup.md");

    for required in [
        "Reserve one 4K-aligned trampoline page below 1 MiB",
        "at least 16 KiB per CPU",
        "Send INIT/SIPI/SIPI",
        "It is not AP startup code and it does not send IPIs",
    ] {
        assert!(
            adr.contains(required),
            "AP startup ADR is missing: {required}"
        );
    }
}

#[test]
fn type1_invariants_doc_covers_memory_wx_rings_and_lifecycle() {
    let doc = read_repo_file("docs/TYPE1_INVARIANTS.md");

    for required in [
        "Memory Ownership",
        "W^X Mapping Intent",
        "Event Ring Loss",
        "CPU And VM State",
        "does not claim the repository boots as a type-1 hypervisor",
    ] {
        assert!(
            doc.contains(required),
            "invariants doc is missing: {required}"
        );
    }
}

#[test]
fn qemu_smoke_script_is_opt_in_and_refuses_missing_boot_images() {
    let script = read_repo_file("scripts/type1-qemu-smoke.sh");
    let testing = read_repo_file("docs/TESTING.md");

    for required in [
        "AEGISHV_TYPE1_BOOT_IMAGE",
        "--print-command",
        "boot image does not exist",
        "expected serial marker was not observed",
        "exit 66",
        "exit 70",
        "command -v \"$qemu\"",
        "qemu-system-x86_64",
        "-kernel \"$image\"",
        "-cdrom \"$image\"",
        "-boot d",
    ] {
        assert!(
            script.contains(required),
            "QEMU smoke script is missing: {required}"
        );
    }

    assert!(testing.contains("scripts/type1-qemu-smoke.sh"));
    assert!(testing.contains("not wired into normal CI"));
}

#[test]
fn vmx_lab_docs_and_script_keep_hardware_scope_explicit() {
    let doc = read_repo_file("docs/VMX_LAB.md");
    let script = read_repo_file("scripts/vmx-linux-lab-smoke.sh");
    let testing = read_repo_file("docs/TESTING.md");

    for required in [
        "Intel VMX Lab Boundary",
        "does not ship a bootable type-1 image",
        "VMXON and VMCS region initialization",
        "Required Exit Coverage",
        "CPUID",
        "Monitor Trap Flag",
        "does not prove that AegisHV boots as a type-1 hypervisor",
    ] {
        assert!(doc.contains(required), "VMX lab doc is missing: {required}");
    }

    for required in [
        "AEGISHV_TYPE1_BOOT_IMAGE",
        "AEGISHV_VMX_LAB_KERNEL",
        "AEGISHV_VMX_LAB_REQUIRE_KVM",
        "/dev/kvm is required",
        "exit 78",
        "host,+vmx",
    ] {
        assert!(
            script.contains(required),
            "VMX Linux lab script is missing: {required}"
        );
    }

    assert!(testing.contains("aegishv-arch-x86::vmx"));
    assert!(testing.contains("do not execute privileged VMX instructions"));
}

#[test]
fn svm_lab_docs_and_script_keep_hardware_scope_explicit() {
    let doc = read_repo_file("docs/SVM_LAB.md");
    let script = read_repo_file("scripts/svm-amd-lab-smoke.sh");
    let testing = read_repo_file("docs/TESTING.md");

    for required in [
        "AMD SVM Lab Boundary",
        "does not ship a bootable type-1 image",
        "VMCB control and state-save structures",
        "Required Intercept Coverage",
        "nested page fault",
        "SEV, SEV-ES, and SEV-SNP",
        "does not prove that AegisHV boots as a type-1 hypervisor",
    ] {
        assert!(doc.contains(required), "SVM lab doc is missing: {required}");
    }

    for required in [
        "AEGISHV_TYPE1_BOOT_IMAGE",
        "AEGISHV_SVM_LAB_KERNEL",
        "AEGISHV_SVM_LAB_REQUIRE_KVM",
        "CPU flags do not report AMD SVM",
        "/dev/kvm is required",
        "host,+svm",
        "exit 78",
    ] {
        assert!(
            script.contains(required),
            "SVM AMD lab script is missing: {required}"
        );
    }

    assert!(testing.contains("aegishv-arch-x86::svm"));
    assert!(testing.contains("do not execute privileged SVM instructions"));
    assert!(testing.contains(".github/workflows/amd-hardware.yml"));
}

#[test]
fn arm64_lab_docs_and_script_keep_hardware_scope_explicit() {
    let doc = read_repo_file("docs/ARM64_LAB.md");
    let script = read_repo_file("scripts/arm64-el2-lab-smoke.sh");
    let testing = read_repo_file("docs/TESTING.md");
    let adr = read_repo_file("docs/adr/0004-arm64-el2-boot.md");
    let gic = read_repo_file("docs/adr/0005-arm64-gic-virtualization.md");
    let index = read_repo_file("docs/adr/README.md");

    for required in [
        "ARM64 EL2 Lab Boundary",
        "does not ship a bootable ARM64 type-1 image",
        "EL2 vector table skeleton",
        "VTCR_EL2 and VTTBR_EL2",
        "does not treat FAR_EL2 as a guest physical address by itself",
        "pKVM, Arm CCA realms",
        "does not prove that AegisHV boots as an ARM64 type-1 hypervisor",
    ] {
        assert!(
            doc.contains(required),
            "ARM64 lab doc is missing: {required}"
        );
    }

    for required in [
        "AEGISHV_ARM64_BOOT_IMAGE",
        "AEGISHV_ARM64_REQUIRE_KVM",
        "/dev/kvm is required",
        "qemu-system-aarch64",
        "virt,virtualization=on,gic-version=3",
        "exit 78",
    ] {
        assert!(
            script.contains(required),
            "ARM64 lab script is missing: {required}"
        );
    }

    assert!(testing.contains("aegishv-arch-arm64"));
    assert!(testing.contains("do not execute privileged EL2 instructions"));
    assert!(adr.contains("firmware or bootloader must enter AegisHV at EL2"));
    assert!(adr.contains("does not claim ARM64 EL2 runtime support"));
    assert!(gic.contains("GICv3 is the preferred target"));
    assert!(gic.contains("does not implement live interrupt injection"));
    assert!(index.contains("ADR-0004"));
    assert!(index.contains("ADR-0005"));
}
