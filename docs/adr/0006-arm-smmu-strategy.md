# ADR-0006: ARM SMMU Isolation Strategy

## Status

Accepted.

## Context

AegisHV now has ARM64 EL2 lab models for Stage-2 page-table planning, TLBI planning, GIC virtualization planning, and protected-guest limits. Device DMA isolation is a separate boundary. A guest memory permission model is not enough when a PCIe or platform device can DMA into memory through an untranslated stream.

ARM systems route device DMA through Stream IDs, SMMU context descriptors, and translation regimes that differ by platform firmware and SMMU generation. Some systems use SMMUv2 context banks. Newer systems usually use SMMUv3 stream tables and command queues. Protected guest modes such as pKVM and Arm CCA can further restrict visibility and ownership.

## Decision

Treat SMMU support as a required isolation proof for ARM64 device assignment. A device may be assigned to a VM only when the backend can prove:

- the device Stream ID is known and stable;
- the Stream ID is bound to a single VM-owned DMA domain;
- Stage-2 translation is enabled for the domain;
- fault reporting is enabled and wired to a bounded event path;
- interrupt remapping or an equivalent interrupt-isolation mechanism is active;
- protected guest memory is not inspected when the platform marks it unavailable.

If any part is missing, the device assignment and DMA map operation must fail closed. The current tree only models this rule in `aegishv-hypervisor-core::iommu`. It does not program SMMU hardware.

## Consequences

The ARM64 device path is explicit about the difference between a model and a live backend. The control plane can reason about Stream IDs and DMA domains without pretending that SMMU programming exists in the current binary.

This also keeps SR-IOV and passthrough claims bounded. A virtual function is not safe just because it has a different PCI function number. It still needs a proven Stream ID or requester ID, DMA isolation, interrupt isolation, and fault handling.

## Test Impact

Unit tests cover fail-closed DMA domain assignment in the core crate. Hardware SMMU programming, command queue handling, and live fault interrupts are not implemented and are not tested.
