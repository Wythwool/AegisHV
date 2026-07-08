#![no_std]

pub const SERIAL_READY_MARKER: &str = "aegishv:type1:halt";
pub const SERIAL_PANIC_MARKER: &str = "aegishv:type1:panic";
pub const LIMINE_BASE_REVISION: u64 = 6;
pub const LIMINE_REQUEST_COUNT: usize = 6;

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
}
