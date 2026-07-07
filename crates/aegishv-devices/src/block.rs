use crate::error::{DeviceError, DeviceErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockRequest {
    pub sector: u64,
    pub sector_count: u32,
    pub write: bool,
}

#[derive(Debug)]
pub struct ReadOnlyBlockImage<'a> {
    bytes: &'a [u8],
    sector_size: u32,
}

impl<'a> ReadOnlyBlockImage<'a> {
    pub fn new(bytes: &'a [u8], sector_size: u32) -> Result<Self, DeviceError> {
        if sector_size == 0
            || !sector_size.is_power_of_two()
            || bytes.len() % sector_size as usize != 0
        {
            return Err(DeviceError::new(
                DeviceErrorKind::OutOfBounds,
                "read-only virtio-blk image must be sector aligned",
            ));
        }
        Ok(Self { bytes, sector_size })
    }

    pub const fn sector_size(&self) -> u32 {
        self.sector_size
    }

    pub fn read<'b>(
        &self,
        request: BlockRequest,
        out: &'b mut [u8],
    ) -> Result<&'b [u8], DeviceError> {
        if request.write {
            return Err(DeviceError::new(
                DeviceErrorKind::Unsupported,
                "read-only virtio-blk image rejects write requests",
            ));
        }
        let bytes = request
            .sector_count
            .checked_mul(self.sector_size)
            .ok_or(DeviceError::new(
                DeviceErrorKind::OutOfBounds,
                "virtio-blk request length overflowed",
            ))? as usize;
        if out.len() < bytes {
            return Err(DeviceError::new(
                DeviceErrorKind::OutOfBounds,
                "virtio-blk output buffer is smaller than the request",
            ));
        }
        let start = request
            .sector
            .checked_mul(self.sector_size as u64)
            .ok_or(DeviceError::new(
                DeviceErrorKind::OutOfBounds,
                "virtio-blk sector offset overflowed",
            ))? as usize;
        let end = start.checked_add(bytes).ok_or(DeviceError::new(
            DeviceErrorKind::OutOfBounds,
            "virtio-blk request end offset overflowed",
        ))?;
        let Some(slice) = self.bytes.get(start..end) else {
            return Err(DeviceError::new(
                DeviceErrorKind::OutOfBounds,
                "virtio-blk request is outside the immutable image",
            ));
        };
        out[..bytes].copy_from_slice(slice);
        Ok(&out[..bytes])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_block_image_reads_sectors_and_rejects_writes() {
        let image = ReadOnlyBlockImage::new(b"aaaabbbb", 4).unwrap();
        let mut out = [0_u8; 4];

        let read = image
            .read(
                BlockRequest {
                    sector: 1,
                    sector_count: 1,
                    write: false,
                },
                &mut out,
            )
            .unwrap();

        assert_eq!(read, b"bbbb");
        assert_eq!(
            image
                .read(
                    BlockRequest {
                        sector: 0,
                        sector_count: 1,
                        write: true,
                    },
                    &mut out,
                )
                .unwrap_err()
                .kind,
            DeviceErrorKind::Unsupported
        );
    }

    #[test]
    fn block_image_rejects_unaligned_images_and_oob_reads() {
        assert_eq!(
            ReadOnlyBlockImage::new(b"abc", 4).unwrap_err().kind,
            DeviceErrorKind::OutOfBounds
        );
        let image = ReadOnlyBlockImage::new(b"aaaabbbb", 4).unwrap();
        let mut out = [0_u8; 4];
        assert_eq!(
            image
                .read(
                    BlockRequest {
                        sector: 2,
                        sector_count: 1,
                        write: false,
                    },
                    &mut out,
                )
                .unwrap_err()
                .kind,
            DeviceErrorKind::OutOfBounds
        );
    }
}
