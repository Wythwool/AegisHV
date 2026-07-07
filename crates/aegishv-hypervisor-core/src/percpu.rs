use crate::error::{CoreError, CoreErrorKind};
use crate::ids::{HostPhysical, PhysicalCpuId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerCpuState {
    Empty,
    Reserved,
    StackReady,
    Online,
    Offline,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PerCpuSlot {
    pub cpu: PhysicalCpuId,
    pub state: PerCpuState,
    pub stack_base: HostPhysical,
    pub stack_size: u64,
    pub event_ring_index: Option<u16>,
    pub scheduler_epoch: u64,
}

pub struct PerCpuTable<const N: usize> {
    slots: [Option<PerCpuSlot>; N],
    len: usize,
}

impl<const N: usize> PerCpuTable<N> {
    pub const fn new() -> Self {
        Self {
            slots: [None; N],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub fn reserve_stack(
        &mut self,
        cpu: PhysicalCpuId,
        stack_base: HostPhysical,
        stack_size: u64,
    ) -> Result<(), CoreError> {
        if self.len >= N {
            return Err(CoreError::new(
                CoreErrorKind::CapacityExceeded,
                "per-cpu table is full",
            ));
        }
        if self.find(cpu).is_some() {
            return Err(CoreError::new(
                CoreErrorKind::InvalidState,
                "per-cpu table already has an entry for this CPU",
            ));
        }
        if stack_size < 16 * 1024 || stack_size % 4096 != 0 || stack_base.get() % 4096 != 0 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "per-cpu stack must be 4K aligned and at least 16 KiB",
            ));
        }

        self.slots[self.len] = Some(PerCpuSlot {
            cpu,
            state: PerCpuState::StackReady,
            stack_base,
            stack_size,
            event_ring_index: None,
            scheduler_epoch: 0,
        });
        self.len += 1;
        Ok(())
    }

    pub fn attach_event_ring(
        &mut self,
        cpu: PhysicalCpuId,
        event_ring_index: u16,
    ) -> Result<(), CoreError> {
        let slot = self.find_mut(cpu)?;
        if slot.state == PerCpuState::Empty || slot.state == PerCpuState::Failed {
            return Err(CoreError::new(
                CoreErrorKind::InvalidState,
                "per-cpu event ring cannot attach to an empty or failed slot",
            ));
        }
        slot.event_ring_index = Some(event_ring_index);
        Ok(())
    }

    pub fn mark_online(&mut self, cpu: PhysicalCpuId) -> Result<(), CoreError> {
        let slot = self.find_mut(cpu)?;
        if slot.state != PerCpuState::StackReady && slot.state != PerCpuState::Offline {
            return Err(CoreError::new(
                CoreErrorKind::InvalidTransition,
                "per-cpu slot can only enter online from stack-ready or offline",
            ));
        }
        if slot.event_ring_index.is_none() {
            return Err(CoreError::new(
                CoreErrorKind::InvalidState,
                "per-cpu slot requires an event ring before it enters online",
            ));
        }
        slot.state = PerCpuState::Online;
        slot.scheduler_epoch += 1;
        Ok(())
    }

    pub fn mark_offline(&mut self, cpu: PhysicalCpuId) -> Result<(), CoreError> {
        let slot = self.find_mut(cpu)?;
        if slot.state != PerCpuState::Online {
            return Err(CoreError::new(
                CoreErrorKind::InvalidTransition,
                "per-cpu slot can only enter offline from online",
            ));
        }
        slot.state = PerCpuState::Offline;
        slot.scheduler_epoch += 1;
        Ok(())
    }

    pub fn get(&self, cpu: PhysicalCpuId) -> Option<PerCpuSlot> {
        self.find(cpu).copied()
    }

    fn find(&self, cpu: PhysicalCpuId) -> Option<&PerCpuSlot> {
        self.slots
            .iter()
            .take(self.len)
            .flatten()
            .find(|slot| slot.cpu == cpu)
    }

    fn find_mut(&mut self, cpu: PhysicalCpuId) -> Result<&mut PerCpuSlot, CoreError> {
        self.slots
            .iter_mut()
            .take(self.len)
            .flatten()
            .find(|slot| slot.cpu == cpu)
            .ok_or(CoreError::new(
                CoreErrorKind::InvalidState,
                "per-cpu table has no entry for this CPU",
            ))
    }
}

impl<const N: usize> Default for PerCpuTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percpu_slot_requires_stack_and_event_ring_before_online() {
        let mut table = PerCpuTable::<2>::new();
        let cpu = PhysicalCpuId::new(1).unwrap();

        table
            .reserve_stack(cpu, HostPhysical::new(0x2000).unwrap(), 16 * 1024)
            .unwrap();
        assert_eq!(
            table.mark_online(cpu).unwrap_err().kind,
            CoreErrorKind::InvalidState
        );

        table.attach_event_ring(cpu, 0).unwrap();
        table.mark_online(cpu).unwrap();

        let slot = table.get(cpu).unwrap();
        assert_eq!(slot.state, PerCpuState::Online);
        assert_eq!(slot.scheduler_epoch, 1);
    }

    #[test]
    fn percpu_table_rejects_duplicate_or_bad_stack() {
        let mut table = PerCpuTable::<2>::new();
        let cpu = PhysicalCpuId::new(0).unwrap();

        assert_eq!(
            table
                .reserve_stack(cpu, HostPhysical::new(0x2001).unwrap(), 16 * 1024)
                .unwrap_err()
                .kind,
            CoreErrorKind::InvalidAddress
        );

        table
            .reserve_stack(cpu, HostPhysical::new(0x4000).unwrap(), 16 * 1024)
            .unwrap();
        assert_eq!(
            table
                .reserve_stack(cpu, HostPhysical::new(0x8000).unwrap(), 16 * 1024)
                .unwrap_err()
                .kind,
            CoreErrorKind::InvalidState
        );
    }
}
