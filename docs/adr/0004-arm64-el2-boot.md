# ADR-0004: ARM64 EL2 Boot Boundary

## Status

Accepted.

## Context

AegisHV now has ARM64 EL2 data models, but it does not ship an ARM64 boot image or an EL2 runtime. ARM64 entry is different from the x86 boot path: firmware may enter at EL1 or EL2, VHE and nVHE have different host register expectations, and the bootloader must preserve device tree and CPU topology information.

## Decision

The ARM64 target requires an explicit boot contract before runtime code may claim EL2 support:

- firmware or bootloader must enter AegisHV at EL2, or must provide a verified transition path to EL2;
- the first ARM64 runtime target is nVHE unless a hardware lab records a VHE-specific reason;
- the bootloader must pass a device tree pointer, memory map, CPU topology, GIC description, timer frequency, and chosen UART;
- the early runtime must install `VBAR_EL2`, configure `HCR_EL2`, `VTCR_EL2`, `VTTBR_EL2`, timer controls, and GIC virtualization state before entering any guest;
- protected guest modes such as pKVM or CCA are treated as visibility boundaries, not as features bypassed by this tree.

This ADR does not make the current binary a type-1 hypervisor and does not claim ARM64 EL2 runtime support.

## Consequences

The repository may model ARM64 EL2 structures and run mock tests before a bootable image exists. Any future runtime work must keep firmware entry, VHE/nVHE selection, and bootloader inputs explicit. Host-side KVM telemetry remains the only active runtime.

## Test Impact

Unit tests cover ARM64 EL2 capability decoding, vector table validation, Stage-2 models, VTCR/VTTBR encoding, ESR decoding, TLBI plans, traps, and virtual timer state. `scripts/arm64-el2-lab-smoke.sh` is opt-in lab plumbing and is not wired into normal CI.
