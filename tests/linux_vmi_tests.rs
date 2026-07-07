use aegishv::linux_vmi::{
    address_in_text_ranges, linux_executable_ranges, resolve_linux_current_task,
    walk_linux_modules, walk_linux_tasks, LinuxWalkLimits, SyntheticLinuxVirtualMemory,
};
use aegishv::vmi::{VcpuId, VmiErrorKind};
use aegishv::vmi_linux_profile::parse_linux_profile;

const INIT_TASK: u64 = 0xffff_8880_0000_1000;
const BASH_TASK: u64 = 0xffff_8880_0000_2000;
const SSHD_TASK: u64 = 0xffff_8880_0000_3000;
const MODULE_HEAD: u64 = 0xffff_8880_0000_5000;
const CURRENT_PTR: u64 = 0xffff_8880_0000_6000;
const KVM_MODULE: u64 = 0xffff_8880_0000_7000;

fn profile_text(extra: &str) -> String {
    format!(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=fixed
symbol=_stext,0xffffffff81000000,0x1000
symbol=_etext,0xffffffff81004000
symbol=init_task,0xffff888000001000
symbol=modules,0xffff888000005000
symbol=aegishv_current_task_vcpu0,0xffff888000006000
offset=task_struct,tasks,0x0,0x10
offset=task_struct,pid,0x20,0x4
offset=task_struct,tgid,0x24,0x4
offset=task_struct,comm,0x30,0x10
offset=task_struct,mm,0x48,0x8
offset=task_struct,cred,0x50,0x8
offset=module,list,0x0,0x10
offset=module,name,0x20,0x20
offset=module,state,0x44,0x4
offset=module,text_base,0x50,0x8
offset=module,text_size,0x58,0x8
syscall=0,read,__x64_sys_read
{extra}
"#
    )
}

fn profile() -> aegishv::vmi_linux_profile::LinuxProfile {
    parse_linux_profile(&profile_text("")).expect("parse linux profile")
}

fn memory() -> SyntheticLinuxVirtualMemory {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    map_task(
        &mut memory,
        TaskFixture {
            address: INIT_TASK,
            next_task: BASH_TASK,
            pid: 1,
            tgid: 1,
            comm: "swapper",
            mm: 0,
            cred: 0x1010,
        },
    );
    map_task(
        &mut memory,
        TaskFixture {
            address: BASH_TASK,
            next_task: SSHD_TASK,
            pid: 1000,
            tgid: 1000,
            comm: "bash",
            mm: 0x4000,
            cred: 0x5010,
        },
    );
    map_task(
        &mut memory,
        TaskFixture {
            address: SSHD_TASK,
            next_task: INIT_TASK,
            pid: 1001,
            tgid: 1001,
            comm: "sshd",
            mm: 0x4100,
            cred: 0x5020,
        },
    );
    map_u64(&mut memory, CURRENT_PTR, SSHD_TASK);
    map_u64(&mut memory, MODULE_HEAD, KVM_MODULE);
    map_module(
        &mut memory,
        KVM_MODULE,
        MODULE_HEAD,
        "kvm",
        0,
        0xffff_ffff_c001_0000,
        0x2000,
    );
    memory
}

struct TaskFixture<'a> {
    address: u64,
    next_task: u64,
    pid: i32,
    tgid: i32,
    comm: &'a str,
    mm: u64,
    cred: u64,
}

fn map_task(memory: &mut SyntheticLinuxVirtualMemory, task: TaskFixture<'_>) {
    let mut bytes = vec![0u8; 0x80];
    write_u64(&mut bytes, 0x0, task.next_task);
    write_u64(&mut bytes, 0x8, 0);
    write_i32(&mut bytes, 0x20, task.pid);
    write_i32(&mut bytes, 0x24, task.tgid);
    write_cstr(&mut bytes, 0x30, 0x10, task.comm);
    write_u64(&mut bytes, 0x48, task.mm);
    write_u64(&mut bytes, 0x50, task.cred);
    memory.map_range(task.address, bytes).expect("map task");
}

fn map_module(
    memory: &mut SyntheticLinuxVirtualMemory,
    address: u64,
    next: u64,
    name: &str,
    state: i32,
    text_base: u64,
    text_size: u64,
) {
    let mut bytes = vec![0u8; 0x80];
    write_u64(&mut bytes, 0x0, next);
    write_u64(&mut bytes, 0x8, 0);
    write_cstr(&mut bytes, 0x20, 0x20, name);
    write_i32(&mut bytes, 0x44, state);
    write_u64(&mut bytes, 0x50, text_base);
    write_u64(&mut bytes, 0x58, text_size);
    memory.map_range(address, bytes).expect("map module");
}

fn map_u64(memory: &mut SyntheticLinuxVirtualMemory, address: u64, value: u64) {
    memory
        .map_range(address, value.to_le_bytes())
        .expect("map pointer");
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_i32(bytes: &mut [u8], offset: usize, value: i32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_cstr(bytes: &mut [u8], offset: usize, len: usize, value: &str) {
    let raw = value.as_bytes();
    assert!(raw.len() < len);
    bytes[offset..offset + raw.len()].copy_from_slice(raw);
}

#[test]
fn walks_init_task_list_and_extracts_process_fields() {
    let tasks = walk_linux_tasks(
        &profile(),
        &memory(),
        0,
        LinuxWalkLimits {
            max_tasks: 8,
            max_modules: 8,
        },
    )
    .expect("walk tasks");

    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].comm, "swapper");
    assert_eq!(tasks[1].pid, 1000);
    assert_eq!(tasks[1].mm, Some(0x4000));
    assert_eq!(tasks[2].comm, "sshd");
    assert_eq!(tasks[2].cred, Some(0x5020));
}

#[test]
fn task_walker_detects_corrupt_loop_before_returning_to_head() {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    map_task(
        &mut memory,
        TaskFixture {
            address: INIT_TASK,
            next_task: BASH_TASK,
            pid: 1,
            tgid: 1,
            comm: "swapper",
            mm: 0,
            cred: 0,
        },
    );
    map_task(
        &mut memory,
        TaskFixture {
            address: BASH_TASK,
            next_task: BASH_TASK,
            pid: 1000,
            tgid: 1000,
            comm: "bash",
            mm: 0,
            cred: 0,
        },
    );

    let err = walk_linux_tasks(
        &profile(),
        &memory,
        0,
        LinuxWalkLimits {
            max_tasks: 8,
            max_modules: 8,
        },
    )
    .expect_err("corrupt loop must fail");

    assert_eq!(err.kind(), VmiErrorKind::InconsistentSnapshot);
    assert!(err.to_string().contains("looped"));
}

#[test]
fn resolves_current_task_from_profile_pointer_symbol() {
    let task = resolve_linux_current_task(&profile(), &memory(), 0, VcpuId(0))
        .expect("resolve current task");

    assert_eq!(task.pid, 1001);
    assert_eq!(task.comm, "sshd");
}

#[test]
fn current_task_resolution_is_explicit_when_profile_lacks_pointer_symbol() {
    let missing = parse_linux_profile(&profile_text(
        "symbol=some_other_pointer,0xffff888000009000\n",
    ))
    .expect("parse profile");

    let err = resolve_linux_current_task(&missing, &memory(), 0, VcpuId(3))
        .expect_err("missing pointer symbol must fail");

    assert_eq!(err.kind(), VmiErrorKind::Unsupported);
    assert!(err.to_string().contains("current-task pointer symbol"));
}

#[test]
fn walks_module_list_and_builds_executable_ranges() {
    let modules = walk_linux_modules(
        &profile(),
        &memory(),
        0,
        LinuxWalkLimits {
            max_tasks: 8,
            max_modules: 8,
        },
    )
    .expect("walk modules");
    let ranges = linux_executable_ranges(&profile(), &modules, 0).expect("ranges");

    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "kvm");
    assert_eq!(modules[0].text_base, 0xffff_ffff_c001_0000);
    assert!(address_in_text_ranges(0xffff_ffff_8100_0100, &ranges).is_some());
    assert_eq!(
        address_in_text_ranges(0xffff_ffff_c001_0100, &ranges)
            .unwrap()
            .owner,
        "kvm"
    );
}

#[test]
fn module_walker_rejects_empty_text_range() {
    let mut memory = SyntheticLinuxVirtualMemory::new();
    map_u64(&mut memory, MODULE_HEAD, KVM_MODULE);
    map_module(
        &mut memory,
        KVM_MODULE,
        MODULE_HEAD,
        "badmod",
        0,
        0xffff_ffff_c002_0000,
        0,
    );

    let err = walk_linux_modules(
        &profile(),
        &memory,
        0,
        LinuxWalkLimits {
            max_tasks: 8,
            max_modules: 8,
        },
    )
    .expect_err("empty module text must fail");

    assert_eq!(err.kind(), VmiErrorKind::InconsistentSnapshot);
    assert!(err.to_string().contains("empty executable text range"));
}
