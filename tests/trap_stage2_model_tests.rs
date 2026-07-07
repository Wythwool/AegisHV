use aegishv::trap::stage2::{
    MemoryType, PageSize, Stage2BackendKind, Stage2Mapping, Stage2Permissions, TrapAccessKind,
};
use aegishv::trap::stage2_model::{synthetic_mapping, Stage2Table};
use aegishv::trap::TrapErrorKind;

#[test]
fn stage2_permissions_track_rw_x_without_arch_assumptions() {
    let perms = Stage2Permissions::READ_WRITE.with_access(TrapAccessKind::Execute, true);

    assert_eq!(perms.compact(), "rwx");
    assert!(perms.is_wx());
    assert!(!perms.without_access(TrapAccessKind::Write).is_wx());
    assert!(perms.allows(TrapAccessKind::Read));
    assert!(perms.allows(TrapAccessKind::Write));
    assert!(perms.allows(TrapAccessKind::Execute));
}

#[test]
fn stage2_backend_limits_keep_synthetic_separate_from_hardware() {
    let synthetic = Stage2BackendKind::Synthetic.limits();
    let ept = Stage2BackendKind::IntelEpt.limits();
    let npt = Stage2BackendKind::AmdNpt.limits();
    let arm = Stage2BackendKind::ArmStage2.limits();

    assert_eq!(synthetic.backend.as_str(), "synthetic");
    assert!(synthetic.note.contains("no hardware permission writes"));
    assert!(ept.memory_types.contains(&MemoryType::WriteProtected));
    assert!(!npt.supports_execute_only);
    assert!(arm.memory_types.contains(&MemoryType::Device));
}

#[test]
fn synthetic_stage2_table_maps_and_updates_permissions() {
    let mut table = Stage2Table::new();
    table
        .map(
            synthetic_mapping(
                "vm-a",
                "cr3:0x1000",
                0x2000,
                PageSize::Size4K,
                Stage2Permissions::READ_EXEC,
            )
            .unwrap(),
        )
        .unwrap();

    let mapping = table.lookup("vm-a", "cr3:0x1000", 0x2abc).unwrap();
    assert_eq!(mapping.base, 0x2000);
    assert_eq!(mapping.permissions, Stage2Permissions::READ_EXEC);

    let old = table
        .set_permissions("vm-a", "cr3:0x1000", 0x2008, Stage2Permissions::READ_WRITE)
        .unwrap();
    assert_eq!(old, Stage2Permissions::READ_EXEC);
    assert_eq!(
        table
            .lookup("vm-a", "cr3:0x1000", 0x2008)
            .unwrap()
            .permissions,
        Stage2Permissions::READ_WRITE
    );
}

#[test]
fn synthetic_stage2_table_rejects_misaligned_and_overlapping_pages() {
    let err = Stage2Mapping::new(
        "vm-a",
        "as0",
        0x2100,
        PageSize::Size4K,
        MemoryType::WriteBack,
        Stage2Permissions::READ,
    )
    .unwrap_err();
    assert_eq!(err.kind(), TrapErrorKind::Misaligned);

    let mut table = Stage2Table::new();
    table
        .map(
            synthetic_mapping(
                "vm-a",
                "as0",
                0x200000,
                PageSize::Size2M,
                Stage2Permissions::READ,
            )
            .unwrap(),
        )
        .unwrap();

    let err = table
        .map(
            synthetic_mapping(
                "vm-a",
                "as0",
                0x201000,
                PageSize::Size4K,
                Stage2Permissions::READ,
            )
            .unwrap(),
        )
        .unwrap_err();
    assert_eq!(err.kind(), TrapErrorKind::Overlap);
}

#[test]
fn synthetic_stage2_table_splits_one_page_table_level_at_a_time() {
    let mut table = Stage2Table::new();
    table
        .map(
            synthetic_mapping(
                "vm-a",
                "as0",
                0x400000,
                PageSize::Size2M,
                Stage2Permissions::READ_EXEC,
            )
            .unwrap(),
        )
        .unwrap();

    let children = table
        .split("vm-a", "as0", 0x400000, PageSize::Size4K)
        .unwrap();

    assert_eq!(children.len(), 512);
    assert_eq!(table.len(), 512);
    assert_eq!(
        table.lookup("vm-a", "as0", 0x401000).unwrap().page_size,
        PageSize::Size4K
    );
    assert_eq!(
        table.lookup("vm-a", "as0", 0x401000).unwrap().permissions,
        Stage2Permissions::READ_EXEC
    );
}

#[test]
fn synthetic_stage2_table_rejects_direct_huge_to_4k_split() {
    let mut table = Stage2Table::new();
    table
        .map(
            synthetic_mapping(
                "vm-a",
                "as0",
                0x40000000,
                PageSize::Size1G,
                Stage2Permissions::READ_EXEC,
            )
            .unwrap(),
        )
        .unwrap();

    let err = table
        .split("vm-a", "as0", 0x40000000, PageSize::Size4K)
        .unwrap_err();

    assert_eq!(err.kind(), TrapErrorKind::UnsupportedCapability);
    assert!(err.detail().contains("one page-table level at a time"));
}
