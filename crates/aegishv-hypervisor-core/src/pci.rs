use crate::error::{CoreError, CoreErrorKind};
use crate::ids::DeviceId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PciBdf {
    pub segment: u16,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

impl PciBdf {
    pub const fn new(segment: u16, bus: u8, device: u8, function: u8) -> Result<Self, CoreError> {
        if device >= 32 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidArgument,
                "PCI device number must be below 32",
            ));
        }
        if function >= 8 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidArgument,
                "PCI function number must be below 8",
            ));
        }
        Ok(Self {
            segment,
            bus,
            device,
            function,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciBarKind {
    Memory32,
    Memory64,
    IoPort,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciBar {
    pub index: u8,
    pub base: u64,
    pub length: u64,
    pub kind: PciBarKind,
    pub prefetchable: bool,
}

impl PciBar {
    pub fn new(
        index: u8,
        base: u64,
        length: u64,
        kind: PciBarKind,
        prefetchable: bool,
    ) -> Result<Self, CoreError> {
        if index >= 6 || length == 0 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidArgument,
                "PCI BAR index and length must describe a real BAR",
            ));
        }
        base.checked_add(length).ok_or(CoreError::new(
            CoreErrorKind::InvalidAddress,
            "PCI BAR address range overflowed",
        ))?;
        Ok(Self {
            index,
            base,
            length,
            kind,
            prefetchable,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MsiCapability {
    pub vectors: u16,
    pub is_msix: bool,
    pub masked: bool,
}

impl MsiCapability {
    pub const fn new(vectors: u16, is_msix: bool, masked: bool) -> Result<Self, CoreError> {
        if vectors == 0 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidArgument,
                "MSI/MSI-X capability must expose at least one vector",
            ));
        }
        Ok(Self {
            vectors,
            is_msix,
            masked,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciDevice {
    pub id: DeviceId,
    pub bdf: PciBdf,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u32,
    pub bars: [Option<PciBar>; 6],
    pub msi: Option<MsiCapability>,
}

impl PciDevice {
    pub const fn new(
        id: DeviceId,
        bdf: PciBdf,
        vendor_id: u16,
        device_id: u16,
        class_code: u32,
    ) -> Self {
        Self {
            id,
            bdf,
            vendor_id,
            device_id,
            class_code,
            bars: [None; 6],
            msi: None,
        }
    }

    pub fn with_bar(mut self, bar: PciBar) -> Result<Self, CoreError> {
        if self.bars[bar.index as usize].is_some() {
            return Err(CoreError::new(
                CoreErrorKind::Overlap,
                "PCI BAR index is already populated",
            ));
        }
        self.bars[bar.index as usize] = Some(bar);
        Ok(self)
    }

    pub fn with_msi(mut self, msi: MsiCapability) -> Self {
        self.msi = Some(msi);
        self
    }
}

pub struct PciInventory<const N: usize> {
    devices: [Option<PciDevice>; N],
    len: usize,
}

impl<const N: usize> PciInventory<N> {
    pub const fn new() -> Self {
        Self {
            devices: [None; N],
            len: 0,
        }
    }

    pub fn devices(&self) -> impl Iterator<Item = PciDevice> + '_ {
        self.devices[..self.len].iter().filter_map(|device| *device)
    }

    pub fn add(&mut self, device: PciDevice) -> Result<(), CoreError> {
        if self.len >= N {
            return Err(CoreError::new(
                CoreErrorKind::CapacityExceeded,
                "PCI inventory is full",
            ));
        }
        if self
            .devices()
            .any(|existing| existing.bdf == device.bdf || existing.id == device.id)
        {
            return Err(CoreError::new(
                CoreErrorKind::Overlap,
                "PCI inventory contains a duplicate BDF or device id",
            ));
        }
        self.devices[self.len] = Some(device);
        self.len += 1;
        Ok(())
    }
}

impl<const N: usize> Default for PciInventory<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pci_inventory_records_bdf_bars_and_msix() {
        let device = PciDevice::new(
            DeviceId::new(1).unwrap(),
            PciBdf::new(0, 0, 3, 0).unwrap(),
            0x1af4,
            0x1001,
            0x010000,
        )
        .with_bar(PciBar::new(0, 0xfebc_0000, 0x1000, PciBarKind::Memory32, false).unwrap())
        .unwrap()
        .with_msi(MsiCapability::new(4, true, true).unwrap());
        let mut inventory = PciInventory::<4>::new();

        inventory.add(device).unwrap();

        let recorded = inventory.devices().next().unwrap();
        assert_eq!(recorded.bdf.device, 3);
        assert_eq!(recorded.bars[0].unwrap().length, 0x1000);
        assert!(recorded.msi.unwrap().is_msix);
    }

    #[test]
    fn pci_inventory_rejects_malformed_and_duplicate_devices() {
        assert_eq!(
            PciBdf::new(0, 0, 32, 0).unwrap_err().kind,
            CoreErrorKind::InvalidArgument
        );
        let device = PciDevice::new(
            DeviceId::new(1).unwrap(),
            PciBdf::new(0, 0, 1, 0).unwrap(),
            0x8086,
            0x100e,
            0x020000,
        );
        let mut inventory = PciInventory::<2>::new();
        inventory.add(device).unwrap();

        assert_eq!(
            inventory.add(device).unwrap_err().kind,
            CoreErrorKind::Overlap
        );
    }
}
