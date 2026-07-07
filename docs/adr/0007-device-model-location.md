# ADR-0007: Device Model Location

## Status

Accepted.

## Context

The repository now has bounded device models for virtio-mmio register state, console queues, read-only block images, and network quarantine policy. These models are useful for lab validation and type-1 boundary design, but they do not run as a device backend.

A planned type-1 design has two common choices for device models:

- keep devices inside the hypervisor runtime;
- move devices into a control VM or service VM and keep the hypervisor runtime smaller.

The first choice has lower IPC cost but a larger trusted computing base in the most privileged runtime. The second choice has more plumbing but gives a clearer blast-radius boundary for parsers, block image handling, network policy, and virtual switch code.

## Decision

AegisHV should keep device emulation out of the hypervisor core by default. The hypervisor runtime should own memory ownership, DMA domains, interrupt routing, and minimal transport state. Rich device behavior belongs in a control VM or service process unless a later backend proves that an in-hypervisor model is small, bounded, and required for boot.

The current `aegishv-devices` crate is a model crate. It is not a live service VM and not a monolithic in-hypervisor device backend.

## Consequences

This keeps parser-heavy code away from the most privileged path. It also means every device operation must carry explicit isolation state. Read-only block devices reject writes. Network quarantine decisions must fail closed when link state or isolation state is not proven.

The cost is more explicit handoff between hypervisor, control plane, and device model code. That is intentional. The repository should pay that cost before claiming device passthrough or virtual switch enforcement.

## Test Impact

Unit tests cover the bounded `aegishv-devices` models. They do not boot a guest, run a service VM, or exercise live MMIO exits.
