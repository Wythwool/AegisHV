use crate::error::{DeviceError, DeviceErrorKind};

pub const VIRTIO_MMIO_MAGIC: u32 = 0x7472_6976;
pub const VIRTIO_MMIO_VERSION_LEGACY: u32 = 1;
pub const VIRTIO_MMIO_VERSION_MODERN: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceStatus {
    Reset,
    Acknowledge,
    Driver,
    FeaturesOk,
    DriverOk,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueLayout {
    pub index: u16,
    pub size: u16,
    pub descriptor: u64,
    pub driver: u64,
    pub device: u64,
}

impl QueueLayout {
    pub fn new(
        index: u16,
        size: u16,
        descriptor: u64,
        driver: u64,
        device: u64,
    ) -> Result<Self, DeviceError> {
        if size == 0 || !size.is_power_of_two() {
            return Err(DeviceError::new(
                DeviceErrorKind::InvalidQueue,
                "virtio queue size must be a non-zero power of two",
            ));
        }
        if descriptor % 16 != 0 || driver % 2 != 0 || device % 4 != 0 {
            return Err(DeviceError::new(
                DeviceErrorKind::InvalidQueue,
                "virtio queue addresses are not aligned for descriptor, driver, and device rings",
            ));
        }
        Ok(Self {
            index,
            size,
            descriptor,
            driver,
            device,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeatureNegotiation {
    pub device_features: u64,
    pub driver_features: u64,
    pub accepted_features: u64,
}

impl FeatureNegotiation {
    pub const fn negotiate(device_features: u64, driver_features: u64) -> Self {
        Self {
            device_features,
            driver_features,
            accepted_features: device_features & driver_features,
        }
    }

    pub const fn contains(self, bit: u8) -> bool {
        bit < 64 && (self.accepted_features & (1_u64 << bit)) != 0
    }
}

pub struct VirtioMmioDevice<const QUEUES: usize> {
    pub device_id: u32,
    pub vendor_id: u32,
    status: DeviceStatus,
    queues: [Option<QueueLayout>; QUEUES],
    queue_len: usize,
    features: FeatureNegotiation,
}

impl<const QUEUES: usize> VirtioMmioDevice<QUEUES> {
    pub const fn new(device_id: u32, vendor_id: u32, device_features: u64) -> Self {
        Self {
            device_id,
            vendor_id,
            status: DeviceStatus::Reset,
            queues: [None; QUEUES],
            queue_len: 0,
            features: FeatureNegotiation {
                device_features,
                driver_features: 0,
                accepted_features: 0,
            },
        }
    }

    pub const fn status(&self) -> DeviceStatus {
        self.status
    }

    pub const fn features(&self) -> FeatureNegotiation {
        self.features
    }

    pub fn set_status(&mut self, status: DeviceStatus) -> Result<(), DeviceError> {
        if self.status == DeviceStatus::Failed && status != DeviceStatus::Reset {
            return Err(DeviceError::new(
                DeviceErrorKind::PermissionDenied,
                "failed virtio device must be reset before reuse",
            ));
        }
        self.status = status;
        Ok(())
    }

    pub fn negotiate_features(&mut self, driver_features: u64) {
        self.features =
            FeatureNegotiation::negotiate(self.features.device_features, driver_features);
    }

    pub fn add_queue(&mut self, queue: QueueLayout) -> Result<(), DeviceError> {
        if self.queue_len >= QUEUES {
            return Err(DeviceError::new(
                DeviceErrorKind::CapacityExceeded,
                "virtio-mmio queue table is full",
            ));
        }
        if self.queues().any(|existing| existing.index == queue.index) {
            return Err(DeviceError::new(
                DeviceErrorKind::InvalidQueue,
                "virtio-mmio queue index is already configured",
            ));
        }
        self.queues[self.queue_len] = Some(queue);
        self.queue_len += 1;
        Ok(())
    }

    pub fn queues(&self) -> impl Iterator<Item = QueueLayout> + '_ {
        self.queues[..self.queue_len]
            .iter()
            .filter_map(|queue| *queue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtio_mmio_negotiates_only_shared_features() {
        let mut device = VirtioMmioDevice::<2>::new(3, 0x554d_4551, 0b1011);

        device.negotiate_features(0b0110);

        assert_eq!(device.features().accepted_features, 0b0010);
        assert!(device.features().contains(1));
        assert!(!device.features().contains(2));
    }

    #[test]
    fn virtio_mmio_rejects_bad_or_duplicate_queues() {
        let mut device = VirtioMmioDevice::<1>::new(3, 1, 0);
        assert_eq!(
            QueueLayout::new(0, 3, 0x1000, 0x2000, 0x3000)
                .unwrap_err()
                .kind,
            DeviceErrorKind::InvalidQueue
        );
        device
            .add_queue(QueueLayout::new(0, 8, 0x1000, 0x2000, 0x3000).unwrap())
            .unwrap();
        assert_eq!(
            device
                .add_queue(QueueLayout::new(1, 8, 0x4000, 0x5000, 0x6000).unwrap())
                .unwrap_err()
                .kind,
            DeviceErrorKind::CapacityExceeded
        );
    }
}
