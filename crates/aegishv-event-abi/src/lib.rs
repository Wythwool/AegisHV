#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![deny(unsafe_code)]

pub use aegishv_hypervisor_core::abi::{
    CommandKind, CommandRecord, CommandRing, EventKind, EventRecord, EventRing, MemoryOrder,
    RingMemoryContract, RING_MEMORY_CONTRACT,
};
pub use aegishv_hypervisor_core::CORE_ABI_VERSION;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AbiPeer {
    pub abi_version: u16,
    pub event_ring_entries: u16,
    pub command_ring_entries: u16,
}

impl AbiPeer {
    pub const fn new(abi_version: u16, event_ring_entries: u16, command_ring_entries: u16) -> Self {
        Self {
            abi_version,
            event_ring_entries,
            command_ring_entries,
        }
    }

    pub const fn is_compatible(self) -> bool {
        self.abi_version == CORE_ABI_VERSION
            && self.event_ring_entries > 0
            && self.command_ring_entries > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_abi_peer_requires_matching_version_and_nonempty_rings() {
        assert!(AbiPeer::new(CORE_ABI_VERSION, 64, 8).is_compatible());
        assert!(!AbiPeer::new(CORE_ABI_VERSION + 1, 64, 8).is_compatible());
        assert!(!AbiPeer::new(CORE_ABI_VERSION, 0, 8).is_compatible());
        assert!(!AbiPeer::new(CORE_ABI_VERSION, 64, 0).is_compatible());
    }

    #[test]
    fn event_abi_reexports_memory_order_contract() {
        assert_eq!(RING_MEMORY_CONTRACT.producer_publish, MemoryOrder::Release);
        assert_eq!(RING_MEMORY_CONTRACT.consumer_observe, MemoryOrder::Acquire);
    }
}
