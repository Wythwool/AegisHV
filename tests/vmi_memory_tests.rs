use aegishv::vmi::{
    GuestMemoryReader, GuestPhysical, MemoryReadError, SyntheticGuestPhysicalMemoryReader, VmId,
    VmiErrorKind,
};

#[test]
fn synthetic_memory_reader_reads_mapped_ranges_without_live_vmi() {
    let mut reader = SyntheticGuestPhysicalMemoryReader::new();
    reader
        .map_range(GuestPhysical(0x1000), vec![0x10, 0x11, 0x12, 0x13])
        .expect("map synthetic range");
    reader
        .map_range(GuestPhysical(0x1004), vec![0x14, 0x15])
        .expect("map adjacent synthetic range");

    let mut buf = [0u8; 5];
    let read = reader
        .read_physical(VmId(1), GuestPhysical(0x1001), &mut buf)
        .expect("read mapped synthetic memory");

    assert_eq!(read, 5);
    assert_eq!(buf, [0x11, 0x12, 0x13, 0x14, 0x15]);
}

#[test]
fn synthetic_memory_reader_rejects_malformed_and_unmapped_reads() {
    let mut reader = SyntheticGuestPhysicalMemoryReader::new();
    reader
        .map_range(GuestPhysical(0x2000), vec![0xaa, 0xbb])
        .expect("map synthetic range");

    let mut empty = [];
    let err = reader
        .read_physical(VmId(1), GuestPhysical(0x2000), &mut empty)
        .expect_err("empty reads are malformed");
    assert!(matches!(
        err,
        MemoryReadError::InvalidRange {
            gpa: GuestPhysical(0x2000),
            len: 0,
        }
    ));

    let mut overflow = [0u8; 2];
    let err = reader
        .read_physical(VmId(1), GuestPhysical(u64::MAX), &mut overflow)
        .expect_err("overflowing address range is invalid");
    assert!(matches!(
        err,
        MemoryReadError::InvalidAddress {
            gpa: GuestPhysical(u64::MAX),
            len: 2,
        }
    ));

    let mut unmapped = [0u8; 4];
    let err = reader
        .read_physical(VmId(1), GuestPhysical(0x3000), &mut unmapped)
        .expect_err("unmapped memory must not return success");
    assert_eq!(err.kind(), VmiErrorKind::Unmapped);
    assert!(err.to_string().contains("gpa=0x3000"));
}

#[test]
fn synthetic_memory_reader_returns_typed_denied_and_unavailable_errors() {
    let mut reader = SyntheticGuestPhysicalMemoryReader::new();
    reader
        .deny_range(
            GuestPhysical(0x4000),
            0x100,
            "MMIO aperture is not readable",
        )
        .expect("mark denied range");
    reader
        .mark_unavailable_range(GuestPhysical(0x5000), 0x100, "offline fixture is missing")
        .expect("mark unavailable range");

    let mut denied = [0u8; 8];
    let err = reader
        .read_physical(VmId(1), GuestPhysical(0x4008), &mut denied)
        .expect_err("denied range must refuse reads");
    assert_eq!(err.kind(), VmiErrorKind::PermissionDenied);
    assert!(err.to_string().contains("MMIO aperture"));

    let mut unavailable = [0u8; 8];
    let err = reader
        .read_physical(VmId(1), GuestPhysical(0x5008), &mut unavailable)
        .expect_err("unavailable range must refuse reads");
    assert_eq!(err.kind(), VmiErrorKind::TemporarilyUnavailable);
    assert!(err.to_string().contains("offline fixture is missing"));
}

#[test]
fn synthetic_memory_reader_requires_explicit_partial_reads() {
    let mut strict = SyntheticGuestPhysicalMemoryReader::new();
    strict
        .map_range(GuestPhysical(0x6000), vec![1, 2])
        .expect("map synthetic range");
    strict
        .deny_range(GuestPhysical(0x6002), 2, "MMIO aperture")
        .expect("mark denied range");

    let mut strict_buf = [0xee; 4];
    let err = strict
        .read_physical(VmId(1), GuestPhysical(0x6000), &mut strict_buf)
        .expect_err("strict read must refuse a denied suffix");
    assert_eq!(err.kind(), VmiErrorKind::PermissionDenied);
    assert_eq!(strict_buf, [0xee; 4]);

    let mut partial = SyntheticGuestPhysicalMemoryReader::new().with_partial_reads(true);
    partial
        .map_range(GuestPhysical(0x6000), vec![1, 2])
        .expect("map synthetic range");
    partial
        .deny_range(GuestPhysical(0x6002), 2, "MMIO aperture")
        .expect("mark denied range");

    let mut partial_buf = [0xee; 4];
    let read = partial
        .read_physical(VmId(1), GuestPhysical(0x6000), &mut partial_buf)
        .expect("explicit partial read returns mapped prefix");
    assert_eq!(read, 2);
    assert_eq!(partial_buf, [1, 2, 0xee, 0xee]);
}

#[test]
fn synthetic_memory_reader_rejects_empty_and_overlapping_ranges() {
    let mut reader = SyntheticGuestPhysicalMemoryReader::new();

    let err = reader
        .map_range(GuestPhysical(0x7000), Vec::<u8>::new())
        .expect_err("empty mapped ranges are invalid");
    assert!(matches!(
        err,
        MemoryReadError::InvalidRange {
            gpa: GuestPhysical(0x7000),
            len: 0,
        }
    ));

    reader
        .map_range(GuestPhysical(0x7000), vec![1, 2, 3, 4])
        .expect("map synthetic range");
    let err = reader
        .mark_unavailable_range(GuestPhysical(0x7002), 4, "overlaps mapped bytes")
        .expect_err("overlapping synthetic ranges are invalid");
    assert!(matches!(
        err,
        MemoryReadError::InvalidRange {
            gpa: GuestPhysical(0x7002),
            len: 4,
        }
    ));
}
