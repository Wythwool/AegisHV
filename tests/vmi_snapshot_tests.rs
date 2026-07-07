use std::path::{Path, PathBuf};

use aegishv::vmi::{GuestMemoryReader, GuestPhysical, MemoryReadError, VmId, VmiErrorKind};
use aegishv::vmi_snapshot::{OfflineGuestMemorySnapshotReader, OFFLINE_MEMORY_SNAPSHOT_FORMAT};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vmi/memory")
}

fn fixture_manifest() -> PathBuf {
    fixture_dir().join("snapshot.map")
}

#[test]
fn offline_snapshot_reader_loads_mapped_fixture_ranges() {
    let reader = OfflineGuestMemorySnapshotReader::from_manifest(fixture_manifest())
        .expect("load offline memory snapshot fixture");

    let mut buf = [0u8; 12];
    let read = reader
        .read_physical(VmId(1), GuestPhysical(0x1000), &mut buf)
        .expect("read mapped offline snapshot range");

    assert_eq!(read, 12);
    assert_eq!(&buf, b"abcdefghijkl");
}

#[test]
fn offline_snapshot_reader_loads_inline_hex_byte_ranges() {
    let manifest = format!("{OFFLINE_MEMORY_SNAPSHOT_FORMAT}\nbytes 0x5000 000102ff\n");
    let reader = OfflineGuestMemorySnapshotReader::from_manifest_text(&manifest, fixture_dir())
        .expect("load inline bytes fixture range");

    let mut buf = [0u8; 4];
    let read = reader
        .read_physical(VmId(1), GuestPhysical(0x5000), &mut buf)
        .expect("read inline bytes range");

    assert_eq!(read, 4);
    assert_eq!(buf, [0x00, 0x01, 0x02, 0xff]);
}

#[test]
fn offline_snapshot_reader_returns_typed_unmapped_denied_and_unavailable_errors() {
    let reader = OfflineGuestMemorySnapshotReader::from_manifest(fixture_manifest())
        .expect("load offline memory snapshot fixture");

    let mut unmapped = [0u8; 4];
    let err = reader
        .read_physical(VmId(1), GuestPhysical(0x1800), &mut unmapped)
        .expect_err("unmapped fixture address must fail");
    assert_eq!(err.kind(), VmiErrorKind::Unmapped);

    let mut denied = [0u8; 4];
    let err = reader
        .read_physical(VmId(1), GuestPhysical(0x2000), &mut denied)
        .expect_err("denied fixture range must fail");
    assert_eq!(err.kind(), VmiErrorKind::PermissionDenied);
    assert!(err.to_string().contains("MMIO aperture"));

    let mut unavailable = [0u8; 4];
    let err = reader
        .read_physical(VmId(1), GuestPhysical(0x3000), &mut unavailable)
        .expect_err("unavailable fixture range must fail");
    assert_eq!(err.kind(), VmiErrorKind::TemporarilyUnavailable);
    assert!(err.to_string().contains("intentionally omitted"));

    let mut missing = [0u8; 4];
    let err = reader
        .read_physical(VmId(1), GuestPhysical(0x4000), &mut missing)
        .expect_err("missing backing file range must fail");
    assert_eq!(err.kind(), VmiErrorKind::TemporarilyUnavailable);
    assert!(err.to_string().contains("missing.mem"));
}

#[test]
fn offline_snapshot_reader_rejects_malformed_manifests() {
    let cases = [
        ("", "missing header"),
        ("aegishv-memory-map-v2\n", "unsupported version"),
        (
            "aegishv-memory-map-v1\nunknown 0x1000 0x4\n",
            "unknown entry kind",
        ),
        (
            "aegishv-memory-map-v1\nmap 0x1000 0x4 pages.mem\n",
            "missing offset",
        ),
        (
            "aegishv-memory-map-v1\nmap nope 0x4 pages.mem 0\n",
            "bad gpa",
        ),
        (
            "aegishv-memory-map-v1\nbytes 0x1000 abc\n",
            "odd inline bytes",
        ),
        (
            "aegishv-memory-map-v1\nbytes 0x1000 zz\n",
            "bad inline bytes",
        ),
    ];

    for (manifest, label) in cases {
        let err = OfflineGuestMemorySnapshotReader::from_manifest_text(manifest, fixture_dir())
            .expect_err(label);
        assert_eq!(err.kind(), VmiErrorKind::Malformed, "{label}");
        assert!(err.to_string().contains("line"), "{label}");
    }
}

#[test]
fn offline_snapshot_reader_rejects_overlapping_and_overflowing_ranges() {
    let overlap = format!(
        "{OFFLINE_MEMORY_SNAPSHOT_FORMAT}\nmap 0x1000 0x4 pages.mem 0\nmap 0x1002 0x4 pages.mem 4\n"
    );
    let err = OfflineGuestMemorySnapshotReader::from_manifest_text(&overlap, fixture_dir())
        .expect_err("overlapping fixture ranges must fail");
    assert!(matches!(
        err,
        MemoryReadError::InvalidRange {
            gpa: GuestPhysical(0x1002),
            len: 4,
        }
    ));

    let overflow =
        format!("{OFFLINE_MEMORY_SNAPSHOT_FORMAT}\ndeny 0xffffffffffffffff 0x2 overflow\n");
    let err = OfflineGuestMemorySnapshotReader::from_manifest_text(&overflow, fixture_dir())
        .expect_err("overflowing guest physical range must fail");
    assert!(matches!(
        err,
        MemoryReadError::InvalidAddress {
            gpa: GuestPhysical(u64::MAX),
            len: 2,
        }
    ));
}

#[test]
fn offline_snapshot_reader_rejects_backing_paths_outside_manifest_dir() {
    let manifest = format!("{OFFLINE_MEMORY_SNAPSHOT_FORMAT}\nmap 0x1000 0x4 ../pages.mem 0\n");
    let err = OfflineGuestMemorySnapshotReader::from_manifest_text(&manifest, fixture_dir())
        .expect_err("parent traversal in backing path must fail");

    assert_eq!(err.kind(), VmiErrorKind::PermissionDenied);
    assert!(err.to_string().contains("manifest directory"));
}
