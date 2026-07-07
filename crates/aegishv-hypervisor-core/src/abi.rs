use crate::error::{CoreError, CoreErrorKind};
use crate::ids::{VcpuId, VmId};
use crate::CORE_ABI_VERSION;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryOrder {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RingMemoryContract {
    pub producer_publish: MemoryOrder,
    pub consumer_observe: MemoryOrder,
}

pub const RING_MEMORY_CONTRACT: RingMemoryContract = RingMemoryContract {
    producer_publish: MemoryOrder::Release,
    consumer_observe: MemoryOrder::Acquire,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    Boot,
    Crash,
    VmState,
    VcpuState,
    Trap,
    Loss,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventRecord {
    pub abi_version: u16,
    pub sequence: u64,
    pub kind: EventKind,
    pub vm_id: Option<VmId>,
    pub vcpu_id: Option<VcpuId>,
    pub arg0: u64,
    pub arg1: u64,
}

impl EventRecord {
    pub const fn empty() -> Self {
        Self {
            abi_version: CORE_ABI_VERSION,
            sequence: 0,
            kind: EventKind::Boot,
            vm_id: None,
            vcpu_id: None,
            arg0: 0,
            arg1: 0,
        }
    }
}

pub struct EventRing<const N: usize> {
    entries: [EventRecord; N],
    head: usize,
    len: usize,
    next_sequence: u64,
    dropped: u64,
}

impl<const N: usize> EventRing<N> {
    pub const fn new() -> Self {
        Self {
            entries: [EventRecord::empty(); N],
            head: 0,
            len: 0,
            next_sequence: 1,
            dropped: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    pub fn push_lossy(
        &mut self,
        kind: EventKind,
        vm_id: Option<VmId>,
        vcpu_id: Option<VcpuId>,
        arg0: u64,
        arg1: u64,
    ) -> Result<EventRecord, CoreError> {
        if N == 0 {
            return Err(CoreError::new(
                CoreErrorKind::CapacityExceeded,
                "event ring capacity must be positive",
            ));
        }

        let record = EventRecord {
            abi_version: CORE_ABI_VERSION,
            sequence: self.next_sequence,
            kind,
            vm_id,
            vcpu_id,
            arg0,
            arg1,
        };
        self.next_sequence = self.next_sequence.checked_add(1).ok_or(CoreError::new(
            CoreErrorKind::InvalidState,
            "event ring sequence counter overflowed",
        ))?;

        if self.len == N {
            self.entries[self.head] = record;
            self.head = (self.head + 1) % N;
            self.dropped += 1;
        } else {
            let index = (self.head + self.len) % N;
            self.entries[index] = record;
            self.len += 1;
        }

        Ok(record)
    }

    pub fn pop(&mut self) -> Option<EventRecord> {
        if self.len == 0 {
            return None;
        }
        let record = self.entries[self.head];
        self.head = (self.head + 1) % N;
        self.len -= 1;
        Some(record)
    }
}

impl<const N: usize> Default for EventRing<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandKind {
    PauseVm,
    ResumeVm,
    StopVm,
    DumpState,
}

impl CommandKind {
    pub const fn from_raw(raw: u16) -> Result<Self, CoreError> {
        match raw {
            1 => Ok(Self::PauseVm),
            2 => Ok(Self::ResumeVm),
            3 => Ok(Self::StopVm),
            4 => Ok(Self::DumpState),
            _ => Err(CoreError::new(
                CoreErrorKind::UnknownCommand,
                "control-plane command code is not supported by this ABI",
            )),
        }
    }

    pub const fn raw(self) -> u16 {
        match self {
            Self::PauseVm => 1,
            Self::ResumeVm => 2,
            Self::StopVm => 3,
            Self::DumpState => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandRecord {
    pub abi_version: u16,
    pub sequence: u64,
    pub kind: CommandKind,
    pub vm_id: VmId,
    pub arg0: u64,
    pub arg1: u64,
}

impl CommandRecord {
    const fn empty() -> Self {
        Self {
            abi_version: CORE_ABI_VERSION,
            sequence: 0,
            kind: CommandKind::DumpState,
            vm_id: VmId::ONE,
            arg0: 0,
            arg1: 0,
        }
    }
}

pub struct CommandRing<const N: usize> {
    entries: [CommandRecord; N],
    head: usize,
    len: usize,
    next_sequence: u64,
}

impl<const N: usize> CommandRing<N> {
    pub const fn new() -> Self {
        Self {
            entries: [CommandRecord::empty(); N],
            head: 0,
            len: 0,
            next_sequence: 1,
        }
    }

    pub fn push_raw(
        &mut self,
        raw_kind: u16,
        vm_id: VmId,
        arg0: u64,
        arg1: u64,
    ) -> Result<CommandRecord, CoreError> {
        let kind = CommandKind::from_raw(raw_kind)?;
        self.push(kind, vm_id, arg0, arg1)
    }

    pub fn push(
        &mut self,
        kind: CommandKind,
        vm_id: VmId,
        arg0: u64,
        arg1: u64,
    ) -> Result<CommandRecord, CoreError> {
        if N == 0 || self.len == N {
            return Err(CoreError::new(
                CoreErrorKind::RingFull,
                "command ring is full",
            ));
        }

        let record = CommandRecord {
            abi_version: CORE_ABI_VERSION,
            sequence: self.next_sequence,
            kind,
            vm_id,
            arg0,
            arg1,
        };
        self.next_sequence = self.next_sequence.checked_add(1).ok_or(CoreError::new(
            CoreErrorKind::InvalidState,
            "command ring sequence counter overflowed",
        ))?;

        let index = (self.head + self.len) % N;
        self.entries[index] = record;
        self.len += 1;
        Ok(record)
    }

    pub fn pop(&mut self) -> Option<CommandRecord> {
        if self.len == 0 {
            return None;
        }
        let record = self.entries[self.head];
        self.head = (self.head + 1) % N;
        self.len -= 1;
        Some(record)
    }
}

impl<const N: usize> Default for CommandRing<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_ring_assigns_sequences_and_records_loss_when_full() {
        let mut ring = EventRing::<2>::new();
        ring.push_lossy(EventKind::Boot, None, None, 1, 0).unwrap();
        ring.push_lossy(EventKind::VmState, Some(VmId::new(7).unwrap()), None, 2, 0)
            .unwrap();
        ring.push_lossy(EventKind::Loss, None, None, 3, 0).unwrap();

        assert_eq!(ring.dropped(), 1);
        assert_eq!(ring.pop().unwrap().sequence, 2);
        assert_eq!(ring.pop().unwrap().sequence, 3);
        assert!(ring.pop().is_none());
    }

    #[test]
    fn command_ring_rejects_unknown_command_before_enqueue() {
        let mut ring = CommandRing::<2>::new();

        assert_eq!(
            ring.push_raw(99, VmId::new(1).unwrap(), 0, 0)
                .unwrap_err()
                .kind,
            CoreErrorKind::UnknownCommand
        );
        assert!(ring.pop().is_none());
    }

    #[test]
    fn command_ring_is_bounded_and_fifo() {
        let mut ring = CommandRing::<1>::new();
        ring.push(CommandKind::PauseVm, VmId::new(1).unwrap(), 0, 0)
            .unwrap();

        assert_eq!(
            ring.push(CommandKind::ResumeVm, VmId::new(1).unwrap(), 0, 0)
                .unwrap_err()
                .kind,
            CoreErrorKind::RingFull
        );
        assert_eq!(ring.pop().unwrap().kind, CommandKind::PauseVm);
    }
}
