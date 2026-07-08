#![no_std]

pub const SERIAL_READY_MARKER: &str = "aegishv:type1:halt";
pub const SERIAL_PANIC_MARKER: &str = "aegishv:type1:panic";

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
}
