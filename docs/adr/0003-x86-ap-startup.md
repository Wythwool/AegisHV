# ADR-0003: x86 Application Processor Startup

## Status

Accepted.

## Context

The x86 type-1 runtime will need each application processor to enter a known state before any scheduler, VM-exit, or trap work can be trusted. The current repository does not contain AP startup assembly or a bootable image.

The model already needs stable boundaries for trampoline placement, per-CPU stack ownership, event buffers, and scheduler attachment. Those boundaries can be tested before SIPI delivery code exists.

## Decision

The bootstrap processor owns AP startup sequencing.

The intended sequence is:

1. Reserve one 4K-aligned trampoline page below 1 MiB.
2. Reserve a separate 4K-aligned stack range for each AP, at least 16 KiB per CPU.
3. Initialize per-CPU state as stack-ready, then attach the CPU's event ring.
4. Send INIT/SIPI/SIPI from the BSP when APIC code exists.
5. Let each AP switch through the trampoline into the common long-mode entry point.
6. Mark the per-CPU slot online only after stack, event ring, and scheduler hooks are present.

`aegishv-arch-x86` contains a validator for the startup plan. It is not AP startup code and it does not send IPIs.

## Consequences

The runtime cannot claim multiprocessor bring-up until there is tested APIC, trampoline, descriptor-table, and entry-code support.

The scheduler model can still reject invalid pCPU/vCPU mappings before that hardware path exists.

## Test Impact

Unit tests cover the AP startup plan constraints: low-memory trampoline, stack size, stack alignment, and nonzero AP count. Later APIC and trampoline code must add hardware-gated tests or opt-in QEMU evidence.
