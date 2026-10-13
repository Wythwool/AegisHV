use aegishv::linux_bpf::{inspect_linux_bpf_programs, LinuxBpfWalkLimits};
use aegishv::linux_vmi::SyntheticLinuxVirtualMemory;
use aegishv::vmi::VmiErrorKind;
use aegishv::vmi_linux_profile::parse_linux_profile;

const BPF_HEAD: u64 = 0xffff_8880_0000_1000;
const BPF_PROG_A: u64 = 0xffff_8880_0000_2000;
const BPF_PROG_B: u64 = 0xffff_8880_0000_2100;
const BPF_AUX_A: u64 = 0xffff_8880_0000_3000;
const JIT_A: u64 = 0xffff_ffff_c020_0000;

fn profile(extra: &str) -> aegishv::vmi_linux_profile::LinuxProfile {
    parse_linux_profile(&format!(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=fixed
symbol=bpf_prog_list,{BPF_HEAD:#x}
offset=bpf_prog,list,0x0,0x10
offset=bpf_prog,aux,0x10,0x8
offset=bpf_prog,type,0x18,0x4
offset=bpf_prog,bpf_func,0x20,0x8
offset=bpf_prog,jited_len,0x28,0x4
offset=bpf_prog_aux,id,0x0,0x4
offset=bpf_prog_aux,name,0x8,0x10
{extra}
"#
    ))
    .expect("parse profile")
}

fn map_u64(memory: &mut SyntheticLinuxVirtualMemory, address: u64, value: u64) {
    memory
        .map_range(address, value.to_le_bytes())
        .expect("map u64");
}

fn map_u32(memory: &mut SyntheticLinuxVirtualMemory, address: u64, value: u32) {
    memory
        .map_range(address, value.to_le_bytes())
        .expect("map u32");
}

fn map_name(memory: &mut SyntheticLinuxVirtualMemory, address: u64, value: &str) {
    let mut bytes = [0u8; 16];
    let raw = value.as_bytes();
    assert!(raw.len() < bytes.len());
    bytes[..raw.len()].copy_from_slice(raw);
    memory.map_range(address, bytes).expect("map name");
}

#[test]
fn bpf_inventory_extracts_program_identity_and_jit_range() {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    map_u64(&mut memory, BPF_HEAD, BPF_PROG_A);
    map_u64(&mut memory, BPF_PROG_A, BPF_HEAD);
    map_u64(&mut memory, BPF_PROG_A + 0x10, BPF_AUX_A);
    map_u32(&mut memory, BPF_PROG_A + 0x18, 7);
    map_u64(&mut memory, BPF_PROG_A + 0x20, JIT_A);
    map_u32(&mut memory, BPF_PROG_A + 0x28, 0x80);
    map_u32(&mut memory, BPF_AUX_A, 42);
    map_name(&mut memory, BPF_AUX_A + 0x8, "audit_prog");

    let inventory =
        inspect_linux_bpf_programs(&profile(""), &memory, 0, LinuxBpfWalkLimits::default())
            .expect("inspect bpf");

    assert_eq!(inventory.programs.len(), 1);
    assert_eq!(inventory.programs[0].id, Some(42));
    assert_eq!(inventory.programs[0].name.as_deref(), Some("audit_prog"));
    assert_eq!(inventory.programs[0].program_type, Some(7));
    assert_eq!(inventory.programs[0].jit_start, Some(JIT_A));
    assert_eq!(inventory.programs[0].jit_end, Some(JIT_A + 0x80));
    assert_eq!(inventory.jit_ranges[0].owner, "bpf:42:audit_prog");
    assert!(inventory.findings.is_empty());
}

#[test]
fn bpf_inventory_reports_unbounded_jit_entry() {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    map_u64(&mut memory, BPF_HEAD, BPF_PROG_A);
    map_u64(&mut memory, BPF_PROG_A, BPF_HEAD);
    map_u64(&mut memory, BPF_PROG_A + 0x10, 0);
    map_u32(&mut memory, BPF_PROG_A + 0x18, 1);
    map_u64(&mut memory, BPF_PROG_A + 0x20, JIT_A);
    map_u32(&mut memory, BPF_PROG_A + 0x28, 0);

    let inventory =
        inspect_linux_bpf_programs(&profile(""), &memory, 0, LinuxBpfWalkLimits::default())
            .expect("inspect bpf");

    assert_eq!(inventory.programs.len(), 1);
    assert_eq!(inventory.programs[0].jit_end, None);
    assert!(inventory.jit_ranges.is_empty());
    assert!(inventory.findings[0].contains("no bounded JIT length"));
}

#[test]
fn bpf_inventory_is_unsupported_without_profile_offsets() {
    let missing = parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=fixed
symbol=bpf_prog_list,0xffff888000001000
"#,
    )
    .expect("parse profile");
    let memory = SyntheticLinuxVirtualMemory::new();

    let err = inspect_linux_bpf_programs(&missing, &memory, 0, LinuxBpfWalkLimits::default())
        .expect_err("missing bpf offsets must be unsupported");

    assert_eq!(err.kind(), VmiErrorKind::Unsupported);
    assert!(err.to_string().contains("bpf_prog.list"));
}

#[test]
fn bpf_inventory_rejects_corrupt_list_loop() {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    map_u64(&mut memory, BPF_HEAD, BPF_PROG_A);
    map_u64(&mut memory, BPF_PROG_A, BPF_PROG_B);
    map_u64(&mut memory, BPF_PROG_A + 0x10, 0);
    map_u32(&mut memory, BPF_PROG_A + 0x18, 1);
    map_u64(&mut memory, BPF_PROG_A + 0x20, 0);
    map_u32(&mut memory, BPF_PROG_A + 0x28, 0);
    map_u64(&mut memory, BPF_PROG_B, BPF_PROG_A);
    map_u64(&mut memory, BPF_PROG_B + 0x10, 0);
    map_u32(&mut memory, BPF_PROG_B + 0x18, 1);
    map_u64(&mut memory, BPF_PROG_B + 0x20, 0);
    map_u32(&mut memory, BPF_PROG_B + 0x28, 0);

    let err = inspect_linux_bpf_programs(&profile(""), &memory, 0, LinuxBpfWalkLimits::default())
        .expect_err("looped list must fail");

    assert_eq!(err.kind(), VmiErrorKind::InconsistentSnapshot);
    assert!(err.to_string().contains("looped"));
}
