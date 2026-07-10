use crate::error::{CoreError, CoreErrorKind};
use crate::ids::{PhysicalCpuId, VcpuId, VmId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VcpuRef {
    pub vm_id: VmId,
    pub vcpu_id: VcpuId,
}

impl VcpuRef {
    pub const fn new(vm_id: VmId, vcpu_id: VcpuId) -> Self {
        Self { vm_id, vcpu_id }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VcpuRunState {
    Runnable,
    Running(PhysicalCpuId),
    Halted,
}

pub struct VcpuScheduler<const Q: usize, const P: usize> {
    queue: [Option<VcpuRef>; Q],
    head: usize,
    len: usize,
    running: [Option<VcpuRef>; P],
    halted: [Option<VcpuRef>; Q],
    halted_len: usize,
}

impl<const Q: usize, const P: usize> VcpuScheduler<Q, P> {
    pub const fn new() -> Self {
        Self {
            queue: [None; Q],
            head: 0,
            len: 0,
            running: [None; P],
            halted: [None; Q],
            halted_len: 0,
        }
    }

    pub const fn queued(&self) -> usize {
        self.len
    }

    pub fn enqueue(&mut self, vcpu: VcpuRef) -> Result<(), CoreError> {
        if self.contains(vcpu) {
            return Err(CoreError::new(
                CoreErrorKind::InvalidState,
                "vCPU is already queued, running, or halted",
            ));
        }
        if self.len == Q {
            return Err(CoreError::new(
                CoreErrorKind::RingFull,
                "vCPU run queue is full",
            ));
        }
        let index = (self.head + self.len) % Q;
        self.queue[index] = Some(vcpu);
        self.len += 1;
        Ok(())
    }

    pub fn schedule_on(&mut self, pcpu: PhysicalCpuId) -> Result<Option<VcpuRef>, CoreError> {
        let index = usize::from(pcpu.get());
        if index >= P {
            return Err(CoreError::new(
                CoreErrorKind::InvalidId,
                "physical CPU is outside the scheduler mapping table",
            ));
        }
        if self.running[index].is_some() {
            return Err(CoreError::new(
                CoreErrorKind::InvalidState,
                "physical CPU already has a running vCPU",
            ));
        }
        let Some(vcpu) = self.pop_queue() else {
            return Ok(None);
        };
        self.running[index] = Some(vcpu);
        Ok(Some(vcpu))
    }

    pub fn halt_running(&mut self, pcpu: PhysicalCpuId) -> Result<VcpuRef, CoreError> {
        let index = usize::from(pcpu.get());
        if index >= P {
            return Err(CoreError::new(
                CoreErrorKind::InvalidId,
                "physical CPU is outside the scheduler mapping table",
            ));
        }
        let vcpu = self.running[index].ok_or(CoreError::new(
            CoreErrorKind::InvalidState,
            "physical CPU has no running vCPU to halt",
        ))?;
        if self.halted_len == Q {
            return Err(CoreError::new(
                CoreErrorKind::CapacityExceeded,
                "halted vCPU table is full",
            ));
        }
        self.halted[self.halted_len] = Some(vcpu);
        self.halted_len += 1;
        self.running[index] = None;
        Ok(vcpu)
    }

    pub fn wake(&mut self, vcpu: VcpuRef) -> Result<(), CoreError> {
        let index = self
            .halted
            .iter()
            .take(self.halted_len)
            .position(|entry| *entry == Some(vcpu))
            .ok_or(CoreError::new(
                CoreErrorKind::InvalidState,
                "vCPU is not halted",
            ))?;
        remove_option(&mut self.halted, &mut self.halted_len, index);
        self.enqueue(vcpu)
    }

    pub fn state_of(&self, vcpu: VcpuRef) -> Option<VcpuRunState> {
        if self.queue.iter().take(Q).any(|entry| *entry == Some(vcpu)) {
            return Some(VcpuRunState::Runnable);
        }
        for (index, entry) in self.running.iter().enumerate() {
            if *entry == Some(vcpu) {
                let pcpu = PhysicalCpuId::new(index as u16).ok()?;
                return Some(VcpuRunState::Running(pcpu));
            }
        }
        if self
            .halted
            .iter()
            .take(self.halted_len)
            .any(|entry| *entry == Some(vcpu))
        {
            return Some(VcpuRunState::Halted);
        }
        None
    }

    fn pop_queue(&mut self) -> Option<VcpuRef> {
        if self.len == 0 {
            return None;
        }
        let vcpu = self.queue[self.head].take();
        self.head = (self.head + 1) % Q;
        self.len -= 1;
        vcpu
    }

    fn contains(&self, vcpu: VcpuRef) -> bool {
        self.queue.iter().any(|entry| *entry == Some(vcpu))
            || self.running.iter().any(|entry| *entry == Some(vcpu))
            || self.halted.iter().any(|entry| *entry == Some(vcpu))
    }
}

impl<const Q: usize, const P: usize> Default for VcpuScheduler<Q, P> {
    fn default() -> Self {
        Self::new()
    }
}

fn remove_option<const N: usize>(
    entries: &mut [Option<VcpuRef>; N],
    len: &mut usize,
    index: usize,
) {
    let mut cursor = index;
    while cursor + 1 < *len {
        entries[cursor] = entries[cursor + 1];
        cursor += 1;
    }
    *len -= 1;
    entries[*len] = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vcpu(raw: u16) -> VcpuRef {
        VcpuRef::new(VmId::new(1).unwrap(), VcpuId::new(raw).unwrap())
    }

    #[test]
    fn scheduler_maps_queued_vcpu_to_physical_cpu() {
        let mut scheduler = VcpuScheduler::<4, 2>::new();
        scheduler.enqueue(vcpu(0)).unwrap();

        let scheduled = scheduler
            .schedule_on(PhysicalCpuId::new(1).unwrap())
            .unwrap()
            .unwrap();

        assert_eq!(scheduled, vcpu(0));
        assert_eq!(
            scheduler.state_of(vcpu(0)),
            Some(VcpuRunState::Running(PhysicalCpuId::new(1).unwrap()))
        );
    }

    #[test]
    fn scheduler_tracks_halt_and_wake_without_preemption_claims() {
        let mut scheduler = VcpuScheduler::<4, 2>::new();
        scheduler.enqueue(vcpu(0)).unwrap();
        scheduler
            .schedule_on(PhysicalCpuId::new(0).unwrap())
            .unwrap();

        scheduler
            .halt_running(PhysicalCpuId::new(0).unwrap())
            .unwrap();
        assert_eq!(scheduler.state_of(vcpu(0)), Some(VcpuRunState::Halted));

        scheduler.wake(vcpu(0)).unwrap();
        assert_eq!(scheduler.state_of(vcpu(0)), Some(VcpuRunState::Runnable));
    }

    #[test]
    fn halt_preserves_running_vcpu_when_halted_table_is_full() {
        let mut scheduler = VcpuScheduler::<1, 2>::new();
        scheduler.enqueue(vcpu(0)).unwrap();
        scheduler
            .schedule_on(PhysicalCpuId::new(0).unwrap())
            .unwrap();
        scheduler
            .halt_running(PhysicalCpuId::new(0).unwrap())
            .unwrap();

        scheduler.enqueue(vcpu(1)).unwrap();
        scheduler
            .schedule_on(PhysicalCpuId::new(1).unwrap())
            .unwrap();

        let error = scheduler
            .halt_running(PhysicalCpuId::new(1).unwrap())
            .unwrap_err();

        assert_eq!(error.kind, CoreErrorKind::CapacityExceeded);
        assert_eq!(scheduler.state_of(vcpu(0)), Some(VcpuRunState::Halted));
        assert_eq!(
            scheduler.state_of(vcpu(1)),
            Some(VcpuRunState::Running(PhysicalCpuId::new(1).unwrap()))
        );
    }

    #[test]
    fn halt_moves_running_vcpu_only_after_capacity_is_available() {
        let mut scheduler = VcpuScheduler::<1, 1>::new();
        scheduler.enqueue(vcpu(0)).unwrap();
        scheduler
            .schedule_on(PhysicalCpuId::new(0).unwrap())
            .unwrap();

        let halted = scheduler
            .halt_running(PhysicalCpuId::new(0).unwrap())
            .unwrap();

        assert_eq!(halted, vcpu(0));
        assert_eq!(scheduler.state_of(vcpu(0)), Some(VcpuRunState::Halted));
        assert_eq!(
            scheduler
                .halt_running(PhysicalCpuId::new(0).unwrap())
                .unwrap_err()
                .kind,
            CoreErrorKind::InvalidState
        );
    }

    #[test]
    fn scheduler_rejects_duplicate_or_invalid_mapping() {
        let mut scheduler = VcpuScheduler::<1, 1>::new();
        scheduler.enqueue(vcpu(0)).unwrap();
        assert_eq!(
            scheduler.enqueue(vcpu(0)).unwrap_err().kind,
            CoreErrorKind::InvalidState
        );
        assert_eq!(
            scheduler
                .schedule_on(PhysicalCpuId::new(3).unwrap())
                .unwrap_err()
                .kind,
            CoreErrorKind::InvalidId
        );
    }
}
