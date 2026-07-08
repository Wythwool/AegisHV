#![no_std]

pub const SERIAL_READY_MARKER: &str = "aegishv:type1:halt";
pub const SERIAL_PANIC_MARKER: &str = "aegishv:type1:panic";
pub const SERIAL_LIMINE_MISSING_MARKER: &str = "aegishv:type1:limine-missing";
pub const SERIAL_LIMINE_BASE_REVISION_MARKER: &str = "aegishv:type1:limine-base-revision";
pub const SERIAL_LIMINE_HHDM_MISSING_MARKER: &str = "aegishv:type1:limine-hhdm-missing";
pub const SERIAL_LIMINE_HHDM_REVISION_MARKER: &str = "aegishv:type1:limine-hhdm-revision";
pub const SERIAL_LIMINE_HHDM_OFFSET_MARKER: &str = "aegishv:type1:limine-hhdm-offset";
pub const SERIAL_LIMINE_MEMMAP_MISSING_MARKER: &str = "aegishv:type1:limine-memmap-missing";
pub const SERIAL_LIMINE_MEMMAP_REVISION_MARKER: &str = "aegishv:type1:limine-memmap-revision";
pub const SERIAL_LIMINE_MEMMAP_EMPTY_MARKER: &str = "aegishv:type1:limine-memmap-empty";
pub const SERIAL_LIMINE_MEMMAP_ENTRIES_MARKER: &str = "aegishv:type1:limine-memmap-entries";
pub const SERIAL_LIMINE_EXECUTABLE_MISSING_MARKER: &str = "aegishv:type1:limine-executable-missing";
pub const SERIAL_LIMINE_EXECUTABLE_REVISION_MARKER: &str =
    "aegishv:type1:limine-executable-revision";
pub const SERIAL_LIMINE_EXECUTABLE_EMPTY_MARKER: &str = "aegishv:type1:limine-executable-empty";
pub const LIMINE_BASE_REVISION: u64 = 6;
pub const LIMINE_REQUEST_COUNT: usize = 6;
pub const LIMINE_RESPONSE_REVISION_OFFSET: usize = 0;
pub const LIMINE_HHDM_OFFSET_OFFSET: usize = 8;
pub const LIMINE_MEMMAP_ENTRY_COUNT_OFFSET: usize = 8;
pub const LIMINE_MEMMAP_ENTRIES_OFFSET: usize = 16;
pub const LIMINE_EXECUTABLE_PHYSICAL_BASE_OFFSET: usize = 8;
pub const LIMINE_EXECUTABLE_VIRTUAL_BASE_OFFSET: usize = 16;

pub const LIMINE_REQUESTS_START_MARKER: [u64; 4] = [
    0xf6b8_f4b3_9de7_d1ae,
    0xfab9_1a69_40fc_b9cf,
    0x785c_6ed0_15d3_e316,
    0x181e_920a_7852_b9d9,
];
pub const LIMINE_REQUESTS_END_MARKER: [u64; 2] = [0xadc0_e053_1bb1_0d03, 0x9572_709f_3176_4c62];

const LIMINE_COMMON_MAGIC: [u64; 2] = [0xc7b1_dd30_df4c_8b88, 0x0a82_e883_a194_f07b];

pub const LIMINE_BOOTLOADER_INFO_REQUEST_ID: [u64; 4] = [
    LIMINE_COMMON_MAGIC[0],
    LIMINE_COMMON_MAGIC[1],
    0xf550_38d8_e2a1_202f,
    0x2794_26fc_f5f5_9740,
];
pub const LIMINE_EXECUTABLE_CMDLINE_REQUEST_ID: [u64; 4] = [
    LIMINE_COMMON_MAGIC[0],
    LIMINE_COMMON_MAGIC[1],
    0x4b16_1536_e598_651e,
    0xb390_ad4a_2f1f_303a,
];
pub const LIMINE_HHDM_REQUEST_ID: [u64; 4] = [
    LIMINE_COMMON_MAGIC[0],
    LIMINE_COMMON_MAGIC[1],
    0x48dc_f1cb_8ad2_b852,
    0x6398_4e95_9a98_244b,
];
pub const LIMINE_MEMMAP_REQUEST_ID: [u64; 4] = [
    LIMINE_COMMON_MAGIC[0],
    LIMINE_COMMON_MAGIC[1],
    0x67cf_3d9d_378a_806f,
    0xe304_acdf_c50c_3c62,
];
pub const LIMINE_RSDP_REQUEST_ID: [u64; 4] = [
    LIMINE_COMMON_MAGIC[0],
    LIMINE_COMMON_MAGIC[1],
    0xc5e7_7b6b_397e_7b43,
    0x2763_7845_accd_cf3c,
];
pub const LIMINE_EXECUTABLE_ADDRESS_REQUEST_ID: [u64; 4] = [
    LIMINE_COMMON_MAGIC[0],
    LIMINE_COMMON_MAGIC[1],
    0x71ba_7686_3cc5_5f63,
    0xb264_4a48_c516_a487,
];

pub const LIMINE_BOOT_REQUEST_IDS: [[u64; 4]; LIMINE_REQUEST_COUNT] = [
    LIMINE_BOOTLOADER_INFO_REQUEST_ID,
    LIMINE_EXECUTABLE_CMDLINE_REQUEST_ID,
    LIMINE_HHDM_REQUEST_ID,
    LIMINE_MEMMAP_REQUEST_ID,
    LIMINE_RSDP_REQUEST_ID,
    LIMINE_EXECUTABLE_ADDRESS_REQUEST_ID,
];

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimineRequest {
    pub id: [u64; 4],
    pub revision: u64,
    pub response: u64,
}

impl LimineRequest {
    pub const fn new(id: [u64; 4]) -> Self {
        Self {
            id,
            revision: 0,
            response: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimineHhdmResponse {
    pub revision: u64,
    pub offset: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimineMemmapResponse {
    pub revision: u64,
    pub entry_count: u64,
    pub entries: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimineExecutableAddressResponse {
    pub revision: u64,
    pub physical_base: u64,
    pub virtual_base: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimineMinimalHandoff {
    pub base_revision_value: u64,
    pub hhdm_response: u64,
    pub hhdm_revision: u64,
    pub hhdm_offset: u64,
    pub memmap_response: u64,
    pub memmap_revision: u64,
    pub memmap_entry_count: u64,
    pub memmap_entries: u64,
    pub executable_address_response: u64,
    pub executable_address_revision: u64,
    pub executable_physical_base: u64,
    pub executable_virtual_base: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimineHandoffStatus {
    Ready,
    BaseRevisionUnsupported,
    HhdmResponseMissing,
    HhdmRevisionUnsupported,
    HhdmOffsetMissing,
    MemmapResponseMissing,
    MemmapRevisionUnsupported,
    MemmapEmpty,
    MemmapEntriesMissing,
    ExecutableAddressResponseMissing,
    ExecutableAddressRevisionUnsupported,
    ExecutableAddressEmpty,
}

impl LimineHandoffStatus {
    pub const fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }

    pub const fn serial_marker(self) -> &'static str {
        match self {
            Self::Ready => SERIAL_READY_MARKER,
            Self::BaseRevisionUnsupported => SERIAL_LIMINE_BASE_REVISION_MARKER,
            Self::HhdmResponseMissing => SERIAL_LIMINE_HHDM_MISSING_MARKER,
            Self::HhdmRevisionUnsupported => SERIAL_LIMINE_HHDM_REVISION_MARKER,
            Self::HhdmOffsetMissing => SERIAL_LIMINE_HHDM_OFFSET_MARKER,
            Self::MemmapResponseMissing => SERIAL_LIMINE_MEMMAP_MISSING_MARKER,
            Self::MemmapRevisionUnsupported => SERIAL_LIMINE_MEMMAP_REVISION_MARKER,
            Self::MemmapEmpty => SERIAL_LIMINE_MEMMAP_EMPTY_MARKER,
            Self::MemmapEntriesMissing => SERIAL_LIMINE_MEMMAP_ENTRIES_MARKER,
            Self::ExecutableAddressResponseMissing => SERIAL_LIMINE_EXECUTABLE_MISSING_MARKER,
            Self::ExecutableAddressRevisionUnsupported => SERIAL_LIMINE_EXECUTABLE_REVISION_MARKER,
            Self::ExecutableAddressEmpty => SERIAL_LIMINE_EXECUTABLE_EMPTY_MARKER,
        }
    }
}

pub const fn limine_minimal_handoff_status(handoff: LimineMinimalHandoff) -> LimineHandoffStatus {
    if handoff.base_revision_value != 0 {
        return LimineHandoffStatus::BaseRevisionUnsupported;
    }
    if handoff.hhdm_response == 0 {
        return LimineHandoffStatus::HhdmResponseMissing;
    }
    if handoff.hhdm_revision != 0 {
        return LimineHandoffStatus::HhdmRevisionUnsupported;
    }
    if handoff.hhdm_offset == 0 {
        return LimineHandoffStatus::HhdmOffsetMissing;
    }
    if handoff.memmap_response == 0 {
        return LimineHandoffStatus::MemmapResponseMissing;
    }
    if handoff.memmap_revision != 0 {
        return LimineHandoffStatus::MemmapRevisionUnsupported;
    }
    if handoff.memmap_entry_count == 0 {
        return LimineHandoffStatus::MemmapEmpty;
    }
    if handoff.memmap_entries == 0 {
        return LimineHandoffStatus::MemmapEntriesMissing;
    }
    if handoff.executable_address_response == 0 {
        return LimineHandoffStatus::ExecutableAddressResponseMissing;
    }
    if handoff.executable_address_revision != 0 {
        return LimineHandoffStatus::ExecutableAddressRevisionUnsupported;
    }
    if handoff.executable_physical_base == 0 || handoff.executable_virtual_base == 0 {
        return LimineHandoffStatus::ExecutableAddressEmpty;
    }
    LimineHandoffStatus::Ready
}

pub const fn limine_base_revision_tag() -> [u64; 3] {
    [
        0xf956_2b2d_5c95_a6c8,
        0x6a7b_3849_4453_6bdc,
        LIMINE_BASE_REVISION,
    ]
}

pub const fn serial_ready_marker() -> &'static str {
    SERIAL_READY_MARKER
}

pub const fn serial_panic_marker() -> &'static str {
    SERIAL_PANIC_MARKER
}

pub const fn serial_limine_missing_marker() -> &'static str {
    SERIAL_LIMINE_MISSING_MARKER
}

pub fn marker_line(marker: &str, out: &mut [u8]) -> Option<usize> {
    let bytes = marker.as_bytes();
    if out.len() < bytes.len() + 1 {
        return None;
    }
    let mut index = 0;
    while index < bytes.len() {
        out[index] = bytes[index];
        index += 1;
    }
    out[index] = b'\n';
    Some(index + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_line_appends_newline_without_allocation() {
        let mut out = [0u8; 32];
        let len = marker_line(SERIAL_READY_MARKER, &mut out).unwrap();

        assert_eq!(&out[..len], b"aegishv:type1:halt\n");
    }

    #[test]
    fn marker_line_rejects_short_buffer() {
        let mut out = [0u8; 4];

        assert_eq!(marker_line(SERIAL_READY_MARKER, &mut out), None);
    }

    #[test]
    fn marker_line_supports_limine_missing_marker() {
        let mut out = [0u8; 40];
        let len = marker_line(SERIAL_LIMINE_MISSING_MARKER, &mut out).unwrap();

        assert_eq!(&out[..len], b"aegishv:type1:limine-missing\n");
    }

    #[test]
    fn handoff_statuses_have_stable_serial_markers() {
        assert_eq!(
            LimineHandoffStatus::Ready.serial_marker(),
            "aegishv:type1:halt"
        );
        assert_eq!(
            LimineHandoffStatus::BaseRevisionUnsupported.serial_marker(),
            "aegishv:type1:limine-base-revision"
        );
        assert_eq!(
            LimineHandoffStatus::HhdmResponseMissing.serial_marker(),
            "aegishv:type1:limine-hhdm-missing"
        );
        assert_eq!(
            LimineHandoffStatus::HhdmRevisionUnsupported.serial_marker(),
            "aegishv:type1:limine-hhdm-revision"
        );
        assert_eq!(
            LimineHandoffStatus::HhdmOffsetMissing.serial_marker(),
            "aegishv:type1:limine-hhdm-offset"
        );
        assert_eq!(
            LimineHandoffStatus::MemmapResponseMissing.serial_marker(),
            "aegishv:type1:limine-memmap-missing"
        );
        assert_eq!(
            LimineHandoffStatus::MemmapRevisionUnsupported.serial_marker(),
            "aegishv:type1:limine-memmap-revision"
        );
        assert_eq!(
            LimineHandoffStatus::MemmapEmpty.serial_marker(),
            "aegishv:type1:limine-memmap-empty"
        );
        assert_eq!(
            LimineHandoffStatus::MemmapEntriesMissing.serial_marker(),
            "aegishv:type1:limine-memmap-entries"
        );
        assert_eq!(
            LimineHandoffStatus::ExecutableAddressResponseMissing.serial_marker(),
            "aegishv:type1:limine-executable-missing"
        );
        assert_eq!(
            LimineHandoffStatus::ExecutableAddressRevisionUnsupported.serial_marker(),
            "aegishv:type1:limine-executable-revision"
        );
        assert_eq!(
            LimineHandoffStatus::ExecutableAddressEmpty.serial_marker(),
            "aegishv:type1:limine-executable-empty"
        );
    }

    #[test]
    fn limine_request_ids_cover_minimal_boot_handoff_inputs() {
        assert_eq!(LIMINE_REQUEST_COUNT, 6);
        assert_eq!(
            LIMINE_MEMMAP_REQUEST_ID,
            [
                0xc7b1_dd30_df4c_8b88,
                0x0a82_e883_a194_f07b,
                0x67cf_3d9d_378a_806f,
                0xe304_acdf_c50c_3c62
            ]
        );
        assert!(LIMINE_BOOT_REQUEST_IDS.contains(&LIMINE_HHDM_REQUEST_ID));
        assert!(LIMINE_BOOT_REQUEST_IDS.contains(&LIMINE_EXECUTABLE_ADDRESS_REQUEST_ID));
    }

    #[test]
    fn limine_base_revision_tag_uses_current_revision() {
        let tag = limine_base_revision_tag();

        assert_eq!(tag[0], 0xf956_2b2d_5c95_a6c8);
        assert_eq!(tag[1], 0x6a7b_3849_4453_6bdc);
        assert_eq!(tag[2], LIMINE_BASE_REVISION);
    }

    #[test]
    fn generic_limine_request_starts_with_id_revision_and_response() {
        let request = LimineRequest::new(LIMINE_RSDP_REQUEST_ID);

        assert_eq!(request.id, LIMINE_RSDP_REQUEST_ID);
        assert_eq!(request.revision, 0);
        assert_eq!(request.response, 0);
        assert_eq!(core::mem::size_of::<LimineRequest>(), 48);
        assert_eq!(core::mem::align_of::<LimineRequest>(), 8);
    }

    #[test]
    fn limine_response_structs_match_expected_offsets() {
        assert_eq!(
            LIMINE_RESPONSE_REVISION_OFFSET,
            core::mem::offset_of!(LimineHhdmResponse, revision)
        );
        assert_eq!(
            LIMINE_HHDM_OFFSET_OFFSET,
            core::mem::offset_of!(LimineHhdmResponse, offset)
        );
        assert_eq!(
            LIMINE_MEMMAP_ENTRY_COUNT_OFFSET,
            core::mem::offset_of!(LimineMemmapResponse, entry_count)
        );
        assert_eq!(
            LIMINE_MEMMAP_ENTRIES_OFFSET,
            core::mem::offset_of!(LimineMemmapResponse, entries)
        );
        assert_eq!(
            LIMINE_EXECUTABLE_PHYSICAL_BASE_OFFSET,
            core::mem::offset_of!(LimineExecutableAddressResponse, physical_base)
        );
        assert_eq!(
            LIMINE_EXECUTABLE_VIRTUAL_BASE_OFFSET,
            core::mem::offset_of!(LimineExecutableAddressResponse, virtual_base)
        );
    }

    #[test]
    fn limine_handoff_status_requires_each_minimal_response() {
        const READY_HANDOFF: LimineMinimalHandoff = LimineMinimalHandoff {
            base_revision_value: 0,
            hhdm_response: 1,
            hhdm_revision: 0,
            hhdm_offset: 0xffff_8000_0000_0000,
            memmap_response: 1,
            memmap_revision: 0,
            memmap_entry_count: 1,
            memmap_entries: 0xffff_8000_0010_0000,
            executable_address_response: 1,
            executable_address_revision: 0,
            executable_physical_base: 0x200000,
            executable_virtual_base: 0xffff_ffff_8020_0000,
        };

        assert!(limine_minimal_handoff_status(READY_HANDOFF).is_ready());

        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                base_revision_value: 6,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::BaseRevisionUnsupported
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                hhdm_response: 0,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::HhdmResponseMissing
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                hhdm_revision: 1,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::HhdmRevisionUnsupported
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                hhdm_offset: 0,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::HhdmOffsetMissing
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                memmap_response: 0,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::MemmapResponseMissing
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                memmap_revision: 1,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::MemmapRevisionUnsupported
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                memmap_entry_count: 0,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::MemmapEmpty
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                memmap_entries: 0,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::MemmapEntriesMissing
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                executable_address_response: 0,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::ExecutableAddressResponseMissing
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                executable_address_revision: 1,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::ExecutableAddressRevisionUnsupported
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                executable_physical_base: 0,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::ExecutableAddressEmpty
        );
    }
}
