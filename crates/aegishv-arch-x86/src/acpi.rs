use aegishv_hypervisor_core::error::{CoreError, CoreErrorKind};

const ACPI_HEADER_LEN: usize = 36;
const DMAR_BODY_LEN: usize = 12;
const IVRS_BODY_LEN: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcpiHeader {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DmarHardwareUnit {
    pub flags: u8,
    pub segment: u16,
    pub register_base: u64,
}

impl DmarHardwareUnit {
    const fn empty() -> Self {
        Self {
            flags: 0,
            segment: 0,
            register_base: 0,
        }
    }
}

#[derive(Debug)]
pub struct DmarTable<const N: usize> {
    pub header: AcpiHeader,
    pub host_address_width: u8,
    pub flags: u8,
    units: [DmarHardwareUnit; N],
    len: usize,
}

impl<const N: usize> DmarTable<N> {
    pub fn units(&self) -> &[DmarHardwareUnit] {
        &self.units[..self.len]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IvrsHardwareUnit {
    pub entry_type: u8,
    pub flags: u8,
    pub device_id: u16,
    pub capability_offset: u16,
    pub base_address: u64,
}

impl IvrsHardwareUnit {
    const fn empty() -> Self {
        Self {
            entry_type: 0,
            flags: 0,
            device_id: 0,
            capability_offset: 0,
            base_address: 0,
        }
    }
}

#[derive(Debug)]
pub struct IvrsTable<const N: usize> {
    pub header: AcpiHeader,
    pub iv_info: u32,
    units: [IvrsHardwareUnit; N],
    len: usize,
}

impl<const N: usize> IvrsTable<N> {
    pub fn units(&self) -> &[IvrsHardwareUnit] {
        &self.units[..self.len]
    }
}

pub fn parse_dmar<const N: usize>(bytes: &[u8]) -> Result<DmarTable<N>, CoreError> {
    let header = parse_header(bytes, *b"DMAR")?;
    let total = checked_table_len(bytes, header.length)?;
    if total < ACPI_HEADER_LEN + DMAR_BODY_LEN {
        return Err(CoreError::new(
            CoreErrorKind::InvalidArgument,
            "DMAR table is shorter than the fixed DMAR body",
        ));
    }
    let host_address_width = bytes[ACPI_HEADER_LEN];
    let flags = bytes[ACPI_HEADER_LEN + 1];
    let mut units = [DmarHardwareUnit::empty(); N];
    let mut len = 0;
    let mut offset = ACPI_HEADER_LEN + DMAR_BODY_LEN;
    while offset < total {
        if offset + 4 > total {
            return Err(CoreError::new(
                CoreErrorKind::InvalidArgument,
                "DMAR remapping structure header is truncated",
            ));
        }
        let structure_type = le_u16(bytes, offset)?;
        let structure_len = le_u16(bytes, offset + 2)? as usize;
        if structure_len < 4 || offset + structure_len > total {
            return Err(CoreError::new(
                CoreErrorKind::InvalidArgument,
                "DMAR remapping structure length is invalid",
            ));
        }
        if structure_type == 0 {
            if structure_len < 16 {
                return Err(CoreError::new(
                    CoreErrorKind::InvalidArgument,
                    "DMAR DRHD structure is truncated",
                ));
            }
            if len >= N {
                return Err(CoreError::new(
                    CoreErrorKind::CapacityExceeded,
                    "DMAR hardware unit table is full",
                ));
            }
            units[len] = DmarHardwareUnit {
                flags: bytes[offset + 4],
                segment: le_u16(bytes, offset + 6)?,
                register_base: le_u64(bytes, offset + 8)?,
            };
            len += 1;
        }
        offset += structure_len;
    }
    Ok(DmarTable {
        header,
        host_address_width,
        flags,
        units,
        len,
    })
}

pub fn parse_ivrs<const N: usize>(bytes: &[u8]) -> Result<IvrsTable<N>, CoreError> {
    let header = parse_header(bytes, *b"IVRS")?;
    let total = checked_table_len(bytes, header.length)?;
    if total < ACPI_HEADER_LEN + IVRS_BODY_LEN {
        return Err(CoreError::new(
            CoreErrorKind::InvalidArgument,
            "IVRS table is shorter than the fixed IVRS body",
        ));
    }
    let iv_info = le_u32(bytes, ACPI_HEADER_LEN)?;
    let mut units = [IvrsHardwareUnit::empty(); N];
    let mut len = 0;
    let mut offset = ACPI_HEADER_LEN + IVRS_BODY_LEN;
    while offset < total {
        if offset + 4 > total {
            return Err(CoreError::new(
                CoreErrorKind::InvalidArgument,
                "IVRS hardware block header is truncated",
            ));
        }
        let entry_type = bytes[offset];
        let flags = bytes[offset + 1];
        let structure_len = le_u16(bytes, offset + 2)? as usize;
        if structure_len < 16 || offset + structure_len > total {
            return Err(CoreError::new(
                CoreErrorKind::InvalidArgument,
                "IVRS hardware block length is invalid",
            ));
        }
        if matches!(entry_type, 0x10 | 0x11 | 0x40) {
            if len >= N {
                return Err(CoreError::new(
                    CoreErrorKind::CapacityExceeded,
                    "IVRS hardware unit table is full",
                ));
            }
            units[len] = IvrsHardwareUnit {
                entry_type,
                flags,
                device_id: le_u16(bytes, offset + 4)?,
                capability_offset: le_u16(bytes, offset + 6)?,
                base_address: le_u64(bytes, offset + 8)?,
            };
            len += 1;
        }
        offset += structure_len;
    }
    Ok(IvrsTable {
        header,
        iv_info,
        units,
        len,
    })
}

fn parse_header(bytes: &[u8], expected: [u8; 4]) -> Result<AcpiHeader, CoreError> {
    if bytes.len() < ACPI_HEADER_LEN {
        return Err(CoreError::new(
            CoreErrorKind::InvalidArgument,
            "ACPI table header is truncated",
        ));
    }
    let mut signature = [0_u8; 4];
    signature.copy_from_slice(&bytes[0..4]);
    if signature != expected {
        return Err(CoreError::new(
            CoreErrorKind::InvalidArgument,
            "ACPI table signature does not match the requested parser",
        ));
    }
    Ok(AcpiHeader {
        signature,
        length: le_u32(bytes, 4)?,
        revision: bytes[8],
    })
}

fn checked_table_len(bytes: &[u8], declared: u32) -> Result<usize, CoreError> {
    let total = declared as usize;
    if total > bytes.len() || total < ACPI_HEADER_LEN {
        return Err(CoreError::new(
            CoreErrorKind::InvalidArgument,
            "ACPI table declared length is outside the supplied fixture",
        ));
    }
    Ok(total)
}

fn le_u16(bytes: &[u8], offset: usize) -> Result<u16, CoreError> {
    let slice = bytes.get(offset..offset + 2).ok_or(CoreError::new(
        CoreErrorKind::InvalidArgument,
        "ACPI fixture ended while reading a u16 field",
    ))?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn le_u32(bytes: &[u8], offset: usize) -> Result<u32, CoreError> {
    let slice = bytes.get(offset..offset + 4).ok_or(CoreError::new(
        CoreErrorKind::InvalidArgument,
        "ACPI fixture ended while reading a u32 field",
    ))?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn le_u64(bytes: &[u8], offset: usize) -> Result<u64, CoreError> {
    let slice = bytes.get(offset..offset + 8).ok_or(CoreError::new(
        CoreErrorKind::InvalidArgument,
        "ACPI fixture ended while reading a u64 field",
    ))?;
    Ok(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(signature: &[u8; 4], len: u32) -> [u8; ACPI_HEADER_LEN] {
        let mut out = [0_u8; ACPI_HEADER_LEN];
        out[0..4].copy_from_slice(signature);
        out[4..8].copy_from_slice(&len.to_le_bytes());
        out[8] = 1;
        out
    }

    #[test]
    fn dmar_parser_extracts_drhd_units_from_fixture() {
        let mut bytes = header(b"DMAR", 64).to_vec();
        bytes.extend_from_slice(&[0x30, 0x1]);
        bytes.extend_from_slice(&[0_u8; 10]);
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.push(1);
        bytes.push(0);
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0xfed9_0000_u64.to_le_bytes());

        let table = parse_dmar::<2>(&bytes).unwrap();

        assert_eq!(table.host_address_width, 0x30);
        assert_eq!(table.units()[0].register_base, 0xfed9_0000);
    }

    #[test]
    fn dmar_parser_rejects_truncated_structure() {
        let mut bytes = header(b"DMAR", 52).to_vec();
        bytes.extend_from_slice(&[0x30, 0]);
        bytes.extend_from_slice(&[0_u8; 10]);
        bytes.extend_from_slice(&0_u16.to_le_bytes());

        assert_eq!(
            parse_dmar::<2>(&bytes).unwrap_err().kind,
            CoreErrorKind::InvalidArgument
        );
    }

    #[test]
    fn ivrs_parser_extracts_hardware_units_from_fixture() {
        let mut bytes = header(b"IVRS", 64).to_vec();
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&[0_u8; 8]);
        bytes.push(0x10);
        bytes.push(0x1);
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(&0x40_u16.to_le_bytes());
        bytes.extend_from_slice(&0x20_u16.to_le_bytes());
        bytes.extend_from_slice(&0xfee0_0000_u64.to_le_bytes());

        let table = parse_ivrs::<2>(&bytes).unwrap();

        assert_eq!(table.iv_info, 1);
        assert_eq!(table.units()[0].device_id, 0x40);
        assert_eq!(table.units()[0].base_address, 0xfee0_0000);
    }

    #[test]
    fn ivrs_parser_rejects_wrong_signature() {
        let bytes = header(b"DMAR", ACPI_HEADER_LEN as u32);

        assert_eq!(
            parse_ivrs::<1>(&bytes).unwrap_err().kind,
            CoreErrorKind::InvalidArgument
        );
    }
}
