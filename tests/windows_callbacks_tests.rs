use aegishv::vmi::VmiErrorKind;
use aegishv::windows_callbacks::{
    inspect_windows_process_callbacks, WindowsCallbackKind, WindowsCallbackWalkLimits,
};
use aegishv::windows_profile::parse_windows_profile;
use aegishv::windows_vmi::{SyntheticWindowsVirtualMemory, WindowsTextRange};

const NT_BASE: u64 = 0xffff_f800_0000_0000;
const CALLBACK_TABLE: u64 = NT_BASE + 0x8000;
const CALLBACK_BLOCK: u64 = 0xffff_8880_0000_1000;
const KERNEL_CALLBACK: u64 = NT_BASE + 0x1200;
const DRIVER_CALLBACK: u64 = 0xffff_f800_0100_0080;
const OUTSIDE_TEXT: u64 = 0xffff_f800_0200_0000;

fn profile(extra: &str) -> aegishv::windows_profile::WindowsProfile {
    parse_windows_profile(&format!(
        r#"
aegishv-windows-profile-v1
os=windows
arch=x86_64
build=10.0.22631.3155
variant=synthetic
pdb_file=ntkrnlmp.pdb
pdb_guid=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
pdb_age=2
symbol=ntoskrnl.exe,0x0,0x4000
symbol=PspCreateProcessNotifyRoutine,0x8000,0x40
offset=EX_CALLBACK_ROUTINE_BLOCK,Function,0x8,0x8
{extra}
"#
    ))
    .expect("parse profile")
}

fn ranges() -> Vec<WindowsTextRange> {
    vec![
        WindowsTextRange {
            owner: "ntoskrnl.exe".to_string(),
            start: NT_BASE,
            end: NT_BASE + 0x4000,
        },
        WindowsTextRange {
            owner: "win32k.sys".to_string(),
            start: 0xffff_f800_0100_0000,
            end: 0xffff_f800_0100_3000,
        },
    ]
}

fn map_u64(memory: &mut SyntheticWindowsVirtualMemory, address: u64, value: u64) {
    memory
        .map_range(address, value.to_le_bytes())
        .expect("map u64");
}

#[test]
fn process_callback_inventory_accepts_known_callback_targets() {
    let mut memory = SyntheticWindowsVirtualMemory::new();
    map_u64(&mut memory, CALLBACK_TABLE, CALLBACK_BLOCK | 0x7);
    map_u64(&mut memory, CALLBACK_BLOCK + 0x8, DRIVER_CALLBACK);

    let report = inspect_windows_process_callbacks(
        &profile(""),
        &memory,
        NT_BASE,
        &ranges(),
        1,
        WindowsCallbackWalkLimits::default(),
    )
    .expect("inspect callbacks");

    assert!(report.ok);
    assert_eq!(report.callbacks.len(), 1);
    assert_eq!(report.callbacks[0].kind, WindowsCallbackKind::ProcessCreate);
    assert_eq!(report.callbacks[0].block_address, CALLBACK_BLOCK);
    assert_eq!(report.callbacks[0].owner.as_deref(), Some("win32k.sys"));
}

#[test]
fn process_callback_inventory_can_read_direct_pointer_entries() {
    let mut memory = SyntheticWindowsVirtualMemory::new();
    let direct_profile = parse_windows_profile(
        r#"
aegishv-windows-profile-v1
os=windows
arch=x86_64
build=10.0.22631.3155
pdb_file=ntkrnlmp.pdb
pdb_guid=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
pdb_age=2
symbol=ntoskrnl.exe,0x0,0x4000
symbol=PspCreateProcessNotifyRoutine,0x8000,0x40
"#,
    )
    .expect("parse profile");
    map_u64(&mut memory, CALLBACK_TABLE, KERNEL_CALLBACK);

    let report = inspect_windows_process_callbacks(
        &direct_profile,
        &memory,
        NT_BASE,
        &ranges(),
        1,
        WindowsCallbackWalkLimits::default(),
    )
    .expect("inspect callbacks");

    assert!(report.ok);
    assert_eq!(report.callbacks[0].callback, KERNEL_CALLBACK);
    assert_eq!(report.callbacks[0].owner.as_deref(), Some("ntoskrnl.exe"));
}

#[test]
fn process_callback_inventory_reports_unknown_targets() {
    let mut memory = SyntheticWindowsVirtualMemory::new();
    map_u64(&mut memory, CALLBACK_TABLE, CALLBACK_BLOCK);
    map_u64(&mut memory, CALLBACK_BLOCK + 0x8, OUTSIDE_TEXT);

    let report = inspect_windows_process_callbacks(
        &profile(""),
        &memory,
        NT_BASE,
        &ranges(),
        1,
        WindowsCallbackWalkLimits::default(),
    )
    .expect("inspect callbacks");

    assert!(!report.ok);
    assert_eq!(report.callbacks[0].owner, None);
    assert!(report.findings[0].contains("outside executable Windows ranges"));
}

#[test]
fn process_callback_inventory_is_unsupported_without_table_symbol() {
    let missing = parse_windows_profile(
        r#"
aegishv-windows-profile-v1
os=windows
arch=x86_64
build=10.0.22631.3155
pdb_file=ntkrnlmp.pdb
pdb_guid=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
pdb_age=2
symbol=ntoskrnl.exe,0x0,0x4000
"#,
    )
    .expect("parse profile");
    let memory = SyntheticWindowsVirtualMemory::new();

    let err = inspect_windows_process_callbacks(
        &missing,
        &memory,
        NT_BASE,
        &ranges(),
        1,
        WindowsCallbackWalkLimits::default(),
    )
    .expect_err("missing callback symbol must fail");

    assert_eq!(err.kind(), VmiErrorKind::Unsupported);
    assert!(err.to_string().contains("PspCreateProcessNotifyRoutine"));
}

#[test]
fn process_callback_inventory_rejects_invalid_slot_count() {
    let memory = SyntheticWindowsVirtualMemory::new();

    let err = inspect_windows_process_callbacks(
        &profile(""),
        &memory,
        NT_BASE,
        &ranges(),
        0,
        WindowsCallbackWalkLimits::default(),
    )
    .expect_err("zero slots must fail");

    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("slot count"));
}
