# Type-1 Invariants

This document records invariants now represented by the `no_std` crates. It does not claim the repository boots as a type-1 hypervisor.

## Memory Ownership

- Firmware memory regions are validated before use.
- Overlapping regions with different types are rejected.
- The physical allocator only allocates 4K pages from usable regions.
- Freeing a page that was not allocated by the allocator is an error.
- Zero-on-alloc requires a caller-provided zeroer; the allocator does not report a page as zeroed without that callback succeeding.

Covered by `aegishv-hypervisor-core::memory` and `aegishv-hypervisor-core::allocator` tests.

## W^X Mapping Intent

- x86 page-table plans reject mappings that are writable and executable.
- Identity and direct-map plans are data models. They do not install CR3 or touch live page tables.

Covered by `aegishv-arch-x86::paging` tests.

## Event Ring Loss

- The hypervisor-to-control-plane event ring is bounded.
- Event records carry ABI version and monotonic sequence numbers.
- When the event ring is full, the oldest entry is overwritten and the loss counter increments.
- The command ring is bounded and rejects unknown command codes before enqueue.

Covered by `aegishv-hypervisor-core::abi` and `aegishv-event-abi` tests.

## CPU And VM State

- A per-CPU slot cannot become online until stack and event ring state exist.
- VM state transitions are explicit: created, configured, runnable, running, paused, stopping, stopped, crashed.
- The vCPU scheduler tracks queued, running, and halted states. It does not implement full preemption.

Covered by `aegishv-hypervisor-core::percpu`, `vm`, and `scheduler` tests.
