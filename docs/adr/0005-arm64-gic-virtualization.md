# ADR-0005: ARM64 GIC Virtualization Strategy

## Status

Accepted.

## Context

ARM64 guest execution needs interrupt virtualization before useful guest scheduling can be claimed. GICv2 and GICv3 expose different control surfaces, and virtual interrupt state must be saved with vCPU state. The current repository only models the boundary.

## Decision

The ARM64 path supports a GIC virtualization plan as data:

- GICv3 is the preferred target when hardware provides it;
- GICv2 remains a compatibility target for older boards;
- each vCPU needs VGIC list register state, priority, group, pending/active bits, and a maintenance interrupt path;
- interrupt injection must be explicit and bounded to valid PPI/SPI interrupt IDs;
- missing GIC virtualization is a typed unsupported condition.

This decision does not implement live interrupt injection and does not claim ARM64 guest runtime support.

## Consequences

The lab model can reject missing or malformed GIC state before any runtime code exists. Future EL2 code must connect VGIC state to vCPU scheduling, timer injection, and device interrupt routing.

## Test Impact

Unit tests cover missing GIC virtualization, empty list-register capacity, and invalid virtual interrupt IDs. Documentation tests require this ADR to remain indexed.
