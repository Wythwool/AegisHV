use aegishv::vmi::{GuestPhysical, GuestVirtual, TranslationError, TranslationResult, VmId};
use aegishv::vmi_cache::{
    AddressSpaceRoot, Arm64CacheGranule, TranslationAccess, TranslationCache,
    TranslationCacheError, TranslationCacheKey, TranslationCacheValue, TranslationMode,
};

const PAGE_4K: u64 = 4096;
const PAGE_2M: u64 = 2 * 1024 * 1024;

fn x86_key(gva: u64) -> TranslationCacheKey {
    TranslationCacheKey::for_gva(
        VmId(1),
        TranslationMode::X86_64FourLevel,
        AddressSpaceRoot::x86_cr3(0x1007),
        GuestVirtual(gva),
        PAGE_4K,
        TranslationAccess::kernel_read(),
    )
    .expect("valid x86 cache key")
}

fn arm64_key(vm: VmId, asid: u16, gva: u64) -> TranslationCacheKey {
    TranslationCacheKey::for_gva(
        vm,
        TranslationMode::Arm64Stage1 {
            granule: Arm64CacheGranule::Size16K,
        },
        AddressSpaceRoot::arm64_ttbr0(0x4000, Some(asid)),
        GuestVirtual(gva),
        16 * 1024,
        TranslationAccess::user_read(),
    )
    .expect("valid ARM64 cache key")
}

fn value(gpa: u64) -> TranslationCacheValue {
    TranslationCacheValue::new(GuestPhysical(gpa), PAGE_4K, true, true, false, true)
        .expect("valid cache value")
}

fn translation(gpa: u64) -> TranslationResult {
    TranslationResult {
        gpa: GuestPhysical(gpa),
        readable: true,
        writable: true,
        user: false,
        executable: true,
        page_size: PAGE_4K,
    }
}

#[test]
fn cache_hit_returns_inserted_translation_for_same_vm_mode_root_and_page() {
    let mut cache = TranslationCache::new(4).expect("valid cache capacity");
    let key = x86_key(0x1234);
    let inserted = value(0x9000);

    cache.insert(key, inserted);

    assert_eq!(cache.lookup(&key), Some(inserted));
    let result = cache
        .lookup_result(&key, GuestVirtual(0x1234))
        .expect("cached translation result");
    assert_eq!(result.gpa, GuestPhysical(0x9234));
    assert_eq!(result.page_size, PAGE_4K);
    assert!(result.readable);
    assert!(result.writable);
    assert!(!result.user);
    assert!(result.executable);
}

#[test]
fn lookup_result_refuses_gva_from_different_virtual_page() {
    let mut cache = TranslationCache::new(4).expect("valid cache capacity");
    let key = x86_key(0x1234);
    cache.insert(key, value(0x9000));

    assert_eq!(cache.lookup(&key), Some(value(0x9000)));
    assert_eq!(cache.lookup_result(&key, GuestVirtual(0x2234)), None);
}

#[test]
fn lookup_result_preserves_offset_for_matching_virtual_page() {
    let mut cache = TranslationCache::new(4).expect("valid cache capacity");
    let key = x86_key(0x1234);
    cache.insert(key, value(0x9000));

    let result = cache
        .lookup_result(&key, GuestVirtual(0x1abc))
        .expect("matching virtual page must hit");

    assert_eq!(result.gpa, GuestPhysical(0x9abc));
    assert_eq!(result.page_size, PAGE_4K);
}

#[test]
fn cache_key_separates_vm_root_mode_page_size_page_and_access() {
    let mut cache = TranslationCache::new(8).expect("valid cache capacity");
    let key = x86_key(0x1234);
    cache.insert(key, value(0x9000));

    let different_vm = TranslationCacheKey { vm: VmId(2), ..key };
    let different_root = TranslationCacheKey {
        root: AddressSpaceRoot::x86_cr3(0x2000),
        ..key
    };
    let different_mode = TranslationCacheKey {
        mode: TranslationMode::X86_64La57,
        ..key
    };
    let different_page_size = TranslationCacheKey::for_gva(
        VmId(1),
        TranslationMode::X86_64FourLevel,
        AddressSpaceRoot::x86_cr3(0x1007),
        GuestVirtual(0x1234),
        PAGE_2M,
        TranslationAccess::kernel_read(),
    )
    .expect("valid 2M cache key");
    let different_page = TranslationCacheKey::for_gva(
        VmId(1),
        TranslationMode::X86_64FourLevel,
        AddressSpaceRoot::x86_cr3(0x1007),
        GuestVirtual(0x2234),
        PAGE_4K,
        TranslationAccess::kernel_read(),
    )
    .expect("valid second page cache key");
    let different_access = TranslationCacheKey {
        access: TranslationAccess::kernel_write(),
        ..key
    };
    let different_arch = TranslationCacheKey {
        mode: TranslationMode::Arm64Stage1 {
            granule: Arm64CacheGranule::Size4K,
        },
        ..key
    };

    assert_eq!(cache.lookup(&different_vm), None);
    assert_eq!(cache.lookup(&different_root), None);
    assert_eq!(cache.lookup(&different_mode), None);
    assert_eq!(cache.lookup(&different_page_size), None);
    assert_eq!(cache.lookup(&different_page), None);
    assert_eq!(cache.lookup(&different_access), None);
    assert_eq!(cache.lookup(&different_arch), None);
}

#[test]
fn cache_capacity_evicts_oldest_entry_deterministically() {
    let mut cache = TranslationCache::new(2).expect("valid cache capacity");
    let first = x86_key(0x1000);
    let second = x86_key(0x2000);
    let third = x86_key(0x3000);

    cache.insert(first, value(0x9000));
    cache.insert(second, value(0xa000));
    assert_eq!(cache.len(), 2);

    cache.insert(third, value(0xb000));

    assert_eq!(cache.capacity(), 2);
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.lookup(&first), None);
    assert_eq!(cache.lookup(&second), Some(value(0xa000)));
    assert_eq!(cache.lookup(&third), Some(value(0xb000)));
}

#[test]
fn inserting_existing_key_updates_value_without_growing_cache() {
    let mut cache = TranslationCache::new(2).expect("valid cache capacity");
    let key = x86_key(0x1000);

    cache.insert(key, value(0x9000));
    cache.insert(key, value(0xa000));

    assert_eq!(cache.len(), 1);
    assert_eq!(cache.lookup(&key), Some(value(0xa000)));
}

#[test]
fn invalid_cache_capacity_and_page_size_return_typed_errors() {
    let err = TranslationCache::new(0).expect_err("zero capacity must fail");
    assert_eq!(err.kind(), aegishv::vmi::VmiErrorKind::InvalidInput);
    assert!(matches!(
        err,
        TranslationCacheError::InvalidCapacity { max_entries: 0 }
    ));

    let err = TranslationCacheKey::for_gva(
        VmId(1),
        TranslationMode::X86_64FourLevel,
        AddressSpaceRoot::x86_cr3(0x1000),
        GuestVirtual(0),
        0,
        TranslationAccess::kernel_read(),
    )
    .expect_err("zero page size must fail");
    assert_eq!(err.kind(), aegishv::vmi::VmiErrorKind::InvalidInput);
    assert!(matches!(
        err,
        TranslationCacheError::InvalidPageSize { page_size: 0 }
    ));
}

#[test]
fn invalidate_by_vmid_removes_only_that_vm() {
    let mut cache = TranslationCache::new(4).expect("valid cache capacity");
    let vm1 = x86_key(0x1000);
    let vm2 = TranslationCacheKey { vm: VmId(2), ..vm1 };
    cache.insert(vm1, value(0x9000));
    cache.insert(vm2, value(0xa000));

    assert_eq!(cache.invalidate_vmid(VmId(1)), 1);

    assert_eq!(cache.lookup(&vm1), None);
    assert_eq!(cache.lookup(&vm2), Some(value(0xa000)));
}

#[test]
fn invalidate_by_cr3_masks_low_bits_and_keeps_other_roots() {
    let mut cache = TranslationCache::new(4).expect("valid cache capacity");
    let cr3_key = x86_key(0x1000);
    let other_cr3 = TranslationCacheKey {
        root: AddressSpaceRoot::x86_cr3(0x2000),
        ..cr3_key
    };
    let arm_root_same_base = TranslationCacheKey {
        mode: TranslationMode::Arm64Stage1 {
            granule: Arm64CacheGranule::Size4K,
        },
        root: AddressSpaceRoot::arm64_ttbr0(0x1000, Some(7)),
        ..cr3_key
    };
    cache.insert(cr3_key, value(0x9000));
    cache.insert(other_cr3, value(0xa000));
    cache.insert(arm_root_same_base, value(0xb000));

    assert_eq!(cache.invalidate_cr3(VmId(1), 0x100f), 1);

    assert_eq!(cache.lookup(&cr3_key), None);
    assert_eq!(cache.lookup(&other_cr3), Some(value(0xa000)));
    assert_eq!(cache.lookup(&arm_root_same_base), Some(value(0xb000)));
}

#[test]
fn invalidate_by_address_space_root_removes_exact_root_only() {
    let mut cache = TranslationCache::new(4).expect("valid cache capacity");
    let ttbr0 = arm64_key(VmId(1), 9, 0x1000);
    let ttbr1 = TranslationCacheKey {
        root: AddressSpaceRoot::arm64_ttbr1(0x4000, Some(9)),
        ..ttbr0
    };
    cache.insert(ttbr0, value(0x9000));
    cache.insert(ttbr1, value(0xa000));

    assert_eq!(
        cache.invalidate_root(VmId(1), AddressSpaceRoot::arm64_ttbr0(0x4000, Some(9))),
        1
    );

    assert_eq!(cache.lookup(&ttbr0), None);
    assert_eq!(cache.lookup(&ttbr1), Some(value(0xa000)));
}

#[test]
fn invalidate_by_asid_removes_only_matching_vm_and_asid() {
    let mut cache = TranslationCache::new(5).expect("valid cache capacity");
    let vm1_asid7 = arm64_key(VmId(1), 7, 0x1000);
    let vm1_asid8 = arm64_key(VmId(1), 8, 0x2000);
    let vm2_asid7 = arm64_key(VmId(2), 7, 0x1000);
    let no_asid = TranslationCacheKey {
        root: AddressSpaceRoot::explicit(0x55, None),
        ..vm1_asid7
    };
    cache.insert(vm1_asid7, value(0x9000));
    cache.insert(vm1_asid8, value(0xa000));
    cache.insert(vm2_asid7, value(0xb000));
    cache.insert(no_asid, value(0xc000));

    assert_eq!(cache.invalidate_asid(VmId(1), 7), 1);

    assert_eq!(cache.lookup(&vm1_asid7), None);
    assert_eq!(cache.lookup(&vm1_asid8), Some(value(0xa000)));
    assert_eq!(cache.lookup(&vm2_asid7), Some(value(0xb000)));
    assert_eq!(cache.lookup(&no_asid), Some(value(0xc000)));
}

#[test]
fn full_flush_removes_all_entries() {
    let mut cache = TranslationCache::new(2).expect("valid cache capacity");
    let first = x86_key(0x1000);
    let second = x86_key(0x2000);
    cache.insert(first, value(0x9000));
    cache.insert(second, value(0xa000));

    assert_eq!(cache.flush(), 2);

    assert!(cache.is_empty());
    assert_eq!(cache.lookup(&first), None);
    assert_eq!(cache.lookup(&second), None);
}

#[test]
fn failed_translation_results_are_not_cached() {
    let mut cache = TranslationCache::new(2).expect("valid cache capacity");
    let key = x86_key(0x1000);
    let failure = TranslationError::NotPresent {
        level: "pte",
        gva: GuestVirtual(0x1000),
    };

    let err = cache
        .insert_translation_result(key, Err(failure.clone()))
        .expect_err("failed translation must stay failed");

    assert_eq!(err, failure);
    assert_eq!(cache.lookup(&key), None);
}

#[test]
fn successful_translation_results_can_be_recorded_as_page_translations() {
    let mut cache = TranslationCache::new(2).expect("valid cache capacity");
    let key = x86_key(0x1234);

    cache
        .insert_translation_result(key, Ok(translation(0x9234)))
        .expect("successful translation can be cached");

    let value = cache.lookup(&key).expect("cached value");
    assert_eq!(value.physical_page, GuestPhysical(0x9000));
    assert_eq!(
        cache
            .lookup_result(&key, GuestVirtual(0x1234))
            .expect("cached translation")
            .gpa,
        GuestPhysical(0x9234)
    );
}

#[test]
fn insert_translation_result_rejects_page_size_mismatch_without_caching() {
    let mut cache = TranslationCache::new(2).expect("valid cache capacity");
    let key = x86_key(0x1234);
    let mismatched = TranslationResult {
        page_size: PAGE_2M,
        ..translation(0x20_1234)
    };

    let err = cache
        .insert_translation_result(key, Ok(mismatched))
        .expect_err("key and value page sizes must match");

    assert!(matches!(err, TranslationError::TranslationFailed { .. }));
    let rendered = err.to_string();
    assert!(rendered.contains("page size 4096"), "{rendered}");
    assert!(rendered.contains("page size 2097152"), "{rendered}");
    assert_eq!(cache.lookup(&key), None);
}
