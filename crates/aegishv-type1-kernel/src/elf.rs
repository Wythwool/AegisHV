pub const READ: u8 = 4;
pub const WRITE: u8 = 2;
pub const EXECUTE: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadError {
    Header,
    Machine,
    Range,
    Alignment,
    Permissions,
    Overlap,
    Entry,
    Dynamic,
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

/// Load a static, identity-linked ELF64 image into a bounded guest allocation.
/// Pages may belong to only one PT_LOAD. Validation finishes before RAM changes.
pub fn load(
    image: &[u8],
    machine: u16,
    base: u64,
    memory: &mut [u8],
    permissions: &mut [u8],
) -> Result<u64, LoadError> {
    if image.len() < 64
        || &image[..7] != b"\x7fELF\x02\x01\x01"
        || u16_at(image, 16) != 2
        || u32_at(image, 20) != 1
        || u16_at(image, 52) != 64
        || u16_at(image, 54) != 56
    {
        return Err(LoadError::Header);
    }
    if u16_at(image, 18) != machine {
        return Err(LoadError::Machine);
    }
    if base & 4095 != 0
        || permissions.is_empty()
        || permissions.len() > 32
        || memory.len() != permissions.len() * 4096
        || base.checked_add(memory.len() as u64).is_none()
    {
        return Err(LoadError::Range);
    }
    let entry = u64_at(image, 24);
    let offset = usize::try_from(u64_at(image, 32)).map_err(|_| LoadError::Header)?;
    let count = usize::from(u16_at(image, 56));
    if count == 0 || count > 128 {
        return Err(LoadError::Header);
    }
    let headers = image
        .get(offset..offset.checked_add(count * 56).ok_or(LoadError::Header)?)
        .ok_or(LoadError::Header)?;
    let mut page_flags = [0u8; 32];
    let mut entry_valid = false;
    for header in headers.chunks_exact(56) {
        match u32_at(header, 0) {
            2 | 3 => return Err(LoadError::Dynamic),
            1 => {}
            _ => continue,
        }
        let flags = u32_at(header, 4);
        let file = u64_at(header, 8);
        let address = u64_at(header, 16);
        let filesz = u64_at(header, 32);
        let memsz = u64_at(header, 40);
        let align = u64_at(header, 48);
        if address != u64_at(header, 24)
            || address & 4095 != 0
            || file & 4095 != 0
            || (align > 1 && (!align.is_power_of_two() || address % align != file % align))
        {
            return Err(LoadError::Alignment);
        }
        if flags & !7 != 0 || flags & u32::from(READ) == 0 || flags & 3 == 3 {
            return Err(LoadError::Permissions);
        }
        if filesz > memsz {
            return Err(LoadError::Range);
        }
        if memsz == 0 {
            continue;
        }
        let start = address.checked_sub(base).ok_or(LoadError::Range)?;
        let end = start.checked_add(memsz).ok_or(LoadError::Range)?;
        if end > memory.len() as u64
            || filesz > memsz
            || file
                .checked_add(filesz)
                .filter(|end| *end <= image.len() as u64)
                .is_none()
        {
            return Err(LoadError::Range);
        }
        for page in &mut page_flags[start as usize / 4096..(end as usize + 4095) / 4096] {
            if *page != 0 {
                return Err(LoadError::Overlap);
            }
            *page = flags as u8;
        }
        if flags & 1 != 0 && entry >= address && entry < address + filesz {
            entry_valid = true;
        }
    }
    if !entry_valid {
        return Err(LoadError::Entry);
    }
    memory.fill(0);
    permissions.copy_from_slice(&page_flags[..permissions.len()]);
    for header in headers.chunks_exact(56) {
        if u32_at(header, 0) != 1 || u64_at(header, 40) == 0 {
            continue;
        }
        let file = u64_at(header, 8) as usize;
        let size = u64_at(header, 32) as usize;
        let target = (u64_at(header, 16) - base) as usize;
        memory[target..target + size].copy_from_slice(&image[file..file + size]);
    }
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> [u8; 8192] {
        let mut bytes = [0u8; 8192];
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        bytes[16..18].copy_from_slice(&2u16.to_le_bytes());
        bytes[18..20].copy_from_slice(&62u16.to_le_bytes());
        bytes[20..24].copy_from_slice(&1u32.to_le_bytes());
        bytes[24..32].copy_from_slice(&0x1000u64.to_le_bytes());
        bytes[32..40].copy_from_slice(&64u64.to_le_bytes());
        bytes[52..54].copy_from_slice(&64u16.to_le_bytes());
        bytes[54..56].copy_from_slice(&56u16.to_le_bytes());
        bytes[56..58].copy_from_slice(&1u16.to_le_bytes());
        bytes[64..68].copy_from_slice(&1u32.to_le_bytes());
        bytes[68..72].copy_from_slice(&5u32.to_le_bytes());
        for offset in [72, 80, 88, 104, 112] {
            bytes[offset..offset + 8].copy_from_slice(&4096u64.to_le_bytes());
        }
        bytes[96..104].copy_from_slice(&3u64.to_le_bytes());
        bytes[4096..4099].copy_from_slice(&[0x31, 0xc0, 0xf4]);
        bytes
    }

    #[test]
    fn loads_text_and_zeros_bss() {
        let mut memory = [0xa5u8; 8192];
        let mut flags = [0; 2];
        assert_eq!(
            load(&image(), 62, 0x1000, &mut memory, &mut flags),
            Ok(0x1000)
        );
        assert_eq!(&memory[..3], &[0x31, 0xc0, 0xf4]);
        assert!(memory[3..].iter().all(|byte| *byte == 0));
        assert_eq!(flags, [READ | EXECUTE, 0]);
    }

    #[test]
    fn rejects_bad_segments_without_touching_guest_memory() {
        for (offset, value, error) in [
            (68, 7, LoadError::Permissions),
            (80, u64::MAX, LoadError::Alignment),
            (96, 8193, LoadError::Range),
            (104, u64::MAX, LoadError::Range),
            (104, 0, LoadError::Range),
            (32, u64::MAX, LoadError::Header),
            (24, 0x1003, LoadError::Entry),
            (24, 0x2000, LoadError::Entry),
        ] {
            let mut bytes = image();
            bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            let mut memory = [0xa5u8; 8192];
            let mut flags = [0xa5; 2];
            assert_eq!(
                load(&bytes, 62, 0x1000, &mut memory, &mut flags),
                Err(error)
            );
            assert!(memory.iter().all(|byte| *byte == 0xa5));
            assert_eq!(flags, [0xa5; 2]);
        }
    }

    #[test]
    fn rejects_overlapping_pages_before_copying() {
        let mut bytes = image();
        bytes[56..58].copy_from_slice(&2u16.to_le_bytes());
        bytes.copy_within(64..120, 120);
        let mut memory = [0xa5; 4096];
        assert_eq!(
            load(&bytes, 62, 0x1000, &mut memory, &mut [0]),
            Err(LoadError::Overlap)
        );
        assert!(memory.iter().all(|byte| *byte == 0xa5));
    }

    #[test]
    fn rejects_truncated_header_and_foreign_machine() {
        let bytes = image();
        for size in 0..120 {
            assert!(load(&bytes[..size], 62, 0x1000, &mut [0; 4096], &mut [0]).is_err());
        }
        assert_eq!(
            load(&bytes, 183, 0x1000, &mut [0; 4096], &mut [0]),
            Err(LoadError::Machine)
        );
    }
}
