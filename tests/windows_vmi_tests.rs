use aegishv::vmi::VmiErrorKind;
use aegishv::vmi_registers::{DescriptorTableRegister, X86_64RegisterSnapshot};
use aegishv::windows_profile::parse_windows_profile;
use aegishv::windows_vmi::{
    address_in_windows_text_ranges, resolve_windows_current_process, resolve_windows_ntoskrnl_base,
    walk_windows_modules, walk_windows_processes, windows_executable_ranges,
    SyntheticWindowsVirtualMemory, WindowsWalkLimits,
};

const NT_BASE: u64 = 0xffff_f800_0000_0000;
const PS_INITIAL: u64 = NT_BASE + 0x3000;
const MODULE_HEAD: u64 = NT_BASE + 0x4000;
const CURRENT_EPROCESS_PTR: u64 = NT_BASE + 0x5000;
const SYSTEM_EPROCESS: u64 = 0xffff_8880_0000_1000;
const USER_EPROCESS: u64 = 0xffff_8880_0000_2000;
const DRIVER_MODULE: u64 = 0xffff_8880_0000_3000;
const DRIVER_BASE: u64 = 0xffff_f800_0100_0000;

fn profile_text(extra: &str) -> String {
    format!(
        r#"
aegishv-windows-profile-v1
os=windows
arch=x86_64
build=10.0.22631.3155
variant=synthetic
pdb_file=ntkrnlmp.pdb
pdb_guid=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
pdb_age=2
symbol=ntoskrnl.exe,0x0,0x400000
symbol=PsInitialSystemProcess,0x3000,0x8
symbol=PsLoadedModuleList,0x4000,0x10
symbol=KiSystemCall64,0x6000,0x80
offset=EPROCESS,ActiveProcessLinks,0x0,0x10
offset=EPROCESS,UniqueProcessId,0x20,0x8
offset=EPROCESS,ImageFileName,0x28,0x10
offset=EPROCESS,DirectoryTableBase,0x38,0x8
offset=KLDR_DATA_TABLE_ENTRY,InLoadOrderLinks,0x0,0x10
offset=KLDR_DATA_TABLE_ENTRY,DllBase,0x20,0x8
offset=KLDR_DATA_TABLE_ENTRY,SizeOfImage,0x28,0x4
offset=KLDR_DATA_TABLE_ENTRY,BaseDllName,0x30,0x40
syscall=0,NtAcceptConnectPort,NtAcceptConnectPort
{extra}
"#
    )
}

fn profile() -> aegishv::windows_profile::WindowsProfile {
    parse_windows_profile(&profile_text("")).expect("parse windows profile")
}

fn memory() -> SyntheticWindowsVirtualMemory {
    let mut memory = SyntheticWindowsVirtualMemory::new();
    map_pe_header(&mut memory, NT_BASE);
    map_u64(&mut memory, PS_INITIAL, SYSTEM_EPROCESS);
    map_process(
        &mut memory,
        ProcessFixture {
            address: SYSTEM_EPROCESS,
            next_process: USER_EPROCESS,
            pid: 4,
            image_name: "System",
            directory_table_base: 0x1000,
        },
    );
    map_process(
        &mut memory,
        ProcessFixture {
            address: USER_EPROCESS,
            next_process: SYSTEM_EPROCESS,
            pid: 1704,
            image_name: "powershell.exe",
            directory_table_base: 0x2200,
        },
    );
    map_u64(&mut memory, CURRENT_EPROCESS_PTR, USER_EPROCESS);
    map_u64(&mut memory, MODULE_HEAD, DRIVER_MODULE);
    map_module(
        &mut memory,
        DRIVER_MODULE,
        MODULE_HEAD,
        "win32k.sys",
        DRIVER_BASE,
        0x3000,
    );
    memory
}

struct ProcessFixture<'a> {
    address: u64,
    next_process: u64,
    pid: u64,
    image_name: &'a str,
    directory_table_base: u64,
}

fn map_process(memory: &mut SyntheticWindowsVirtualMemory, process: ProcessFixture<'_>) {
    let mut bytes = vec![0u8; 0x60];
    write_u64(&mut bytes, 0x0, process.next_process);
    write_u64(&mut bytes, 0x8, 0);
    write_u64(&mut bytes, 0x20, process.pid);
    write_cstr(&mut bytes, 0x28, 0x10, process.image_name);
    write_u64(&mut bytes, 0x38, process.directory_table_base);
    memory
        .map_range(process.address, bytes)
        .expect("map eprocess");
}

fn map_module(
    memory: &mut SyntheticWindowsVirtualMemory,
    address: u64,
    next: u64,
    name: &str,
    dll_base: u64,
    size_of_image: u32,
) {
    let mut bytes = vec![0u8; 0x80];
    write_u64(&mut bytes, 0x0, next);
    write_u64(&mut bytes, 0x8, 0);
    write_u64(&mut bytes, 0x20, dll_base);
    write_u32(&mut bytes, 0x28, size_of_image);
    write_utf16(&mut bytes, 0x30, 0x40, name);
    memory.map_range(address, bytes).expect("map module");
}

fn map_pe_header(memory: &mut SyntheticWindowsVirtualMemory, address: u64) {
    let mut bytes = vec![0u8; 0x90];
    bytes[0..2].copy_from_slice(b"MZ");
    write_u32(&mut bytes, 0x3c, 0x80);
    bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
    memory.map_range(address, bytes).expect("map PE header");
}

fn map_u64(memory: &mut SyntheticWindowsVirtualMemory, address: u64, value: u64) {
    memory
        .map_range(address, value.to_le_bytes())
        .expect("map pointer");
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_cstr(bytes: &mut [u8], offset: usize, len: usize, value: &str) {
    let raw = value.as_bytes();
    assert!(raw.len() < len);
    bytes[offset..offset + raw.len()].copy_from_slice(raw);
}

fn write_utf16(bytes: &mut [u8], offset: usize, len: usize, value: &str) {
    let words: Vec<u16> = value.encode_utf16().collect();
    assert!((words.len() + 1) * 2 <= len);
    for (index, word) in words.iter().enumerate() {
        let at = offset + index * 2;
        bytes[at..at + 2].copy_from_slice(&word.to_le_bytes());
    }
}

fn registers_with_cr3(cr3: u64) -> X86_64RegisterSnapshot {
    X86_64RegisterSnapshot::new(
        0,
        0,
        cr3,
        0,
        0,
        DescriptorTableRegister::new(0, 0),
        DescriptorTableRegister::new(0, 0),
    )
}

#[test]
fn ntoskrnl_base_resolver_accepts_one_pe_candidate() {
    let memory = memory();

    let base = resolve_windows_ntoskrnl_base(&memory, &[0, NT_BASE, NT_BASE + 0x1000])
        .expect("resolve nt base");

    assert_eq!(base, NT_BASE);
}

#[test]
fn ntoskrnl_base_resolver_rejects_missing_and_ambiguous_candidates() {
    let mut memory = SyntheticWindowsVirtualMemory::new();
    let err = resolve_windows_ntoskrnl_base(&memory, &[NT_BASE]).expect_err("missing PE");
    assert_eq!(err.kind(), VmiErrorKind::InconsistentSnapshot);

    map_pe_header(&mut memory, NT_BASE);
    map_pe_header(&mut memory, NT_BASE + 0x10_0000);
    let err = resolve_windows_ntoskrnl_base(&memory, &[NT_BASE, NT_BASE + 0x10_0000])
        .expect_err("ambiguous PE");
    assert_eq!(err.kind(), VmiErrorKind::InconsistentSnapshot);
    assert!(err.to_string().contains("more than one"));
}

#[test]
fn walks_eprocess_list_and_resolves_current_process_by_cr3() {
    let processes = walk_windows_processes(
        &profile(),
        &memory(),
        NT_BASE,
        WindowsWalkLimits {
            max_processes: 8,
            max_modules: 8,
        },
    )
    .expect("walk processes");

    assert_eq!(processes.len(), 2);
    assert_eq!(processes[0].pid, 4);
    assert_eq!(processes[0].image_name, "System");
    assert_eq!(processes[1].pid, 1704);
    assert_eq!(processes[1].directory_table_base, Some(0x2200));

    let current = resolve_windows_current_process(
        &profile(),
        &memory(),
        NT_BASE,
        &registers_with_cr3(0x2200),
        WindowsWalkLimits {
            max_processes: 8,
            max_modules: 8,
        },
    )
    .expect("resolve current process");

    assert_eq!(current.image_name, "powershell.exe");
}

#[test]
fn current_process_can_use_explicit_pointer_symbol() {
    let profile = parse_windows_profile(&profile_text(
        "symbol=aegishv_current_eprocess,0x5000,0x8\n",
    ))
    .expect("parse profile");
    let current = resolve_windows_current_process(
        &profile,
        &memory(),
        NT_BASE,
        &registers_with_cr3(0xdead),
        WindowsWalkLimits {
            max_processes: 8,
            max_modules: 8,
        },
    )
    .expect("resolve current process");

    assert_eq!(current.pid, 1704);
}

#[test]
fn process_walker_rejects_corrupt_loop() {
    let mut memory = SyntheticWindowsVirtualMemory::new();
    map_u64(&mut memory, PS_INITIAL, SYSTEM_EPROCESS);
    map_process(
        &mut memory,
        ProcessFixture {
            address: SYSTEM_EPROCESS,
            next_process: USER_EPROCESS,
            pid: 4,
            image_name: "System",
            directory_table_base: 0x1000,
        },
    );
    map_process(
        &mut memory,
        ProcessFixture {
            address: USER_EPROCESS,
            next_process: USER_EPROCESS,
            pid: 1704,
            image_name: "powershell.exe",
            directory_table_base: 0x2200,
        },
    );

    let err = walk_windows_processes(
        &profile(),
        &memory,
        NT_BASE,
        WindowsWalkLimits {
            max_processes: 8,
            max_modules: 8,
        },
    )
    .expect_err("corrupt loop must fail");

    assert_eq!(err.kind(), VmiErrorKind::InconsistentSnapshot);
    assert!(err.to_string().contains("looped"));
}

#[test]
fn walks_loaded_module_list_and_builds_executable_ranges() {
    let modules = walk_windows_modules(
        &profile(),
        &memory(),
        NT_BASE,
        WindowsWalkLimits {
            max_processes: 8,
            max_modules: 8,
        },
    )
    .expect("walk modules");
    let ranges = windows_executable_ranges(&profile(), &modules, NT_BASE).expect("ranges");

    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "win32k.sys");
    assert_eq!(modules[0].dll_base, DRIVER_BASE);
    assert!(address_in_windows_text_ranges(NT_BASE + 0x1000, &ranges).is_some());
    assert_eq!(
        address_in_windows_text_ranges(DRIVER_BASE + 0x1200, &ranges)
            .unwrap()
            .owner,
        "win32k.sys"
    );
}

#[test]
fn module_walker_rejects_empty_image_range() {
    let mut memory = SyntheticWindowsVirtualMemory::new();
    map_u64(&mut memory, MODULE_HEAD, DRIVER_MODULE);
    map_module(
        &mut memory,
        DRIVER_MODULE,
        MODULE_HEAD,
        "bad.sys",
        DRIVER_BASE,
        0,
    );

    let err = walk_windows_modules(
        &profile(),
        &memory,
        NT_BASE,
        WindowsWalkLimits {
            max_processes: 8,
            max_modules: 8,
        },
    )
    .expect_err("empty module must fail");

    assert_eq!(err.kind(), VmiErrorKind::InconsistentSnapshot);
    assert!(err.to_string().contains("empty image range"));
}
