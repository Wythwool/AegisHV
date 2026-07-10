# ADR-0003: x86 Application Processor Startup

## Status

Accepted.

## Context

The x86 Type-1 lab runtime now has a bootable Limine ISO, owned BSP descriptor tables and stacks, and a wired Intel VMX toy-guest path. It remains BSP-only. Each application processor will need to enter a known state before any scheduler, VM-exit, interrupt, or trap work can be trusted.

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

The current boot and VMX paths establish no SMP coverage. The runtime cannot claim multiprocessor bring-up until there is tested APIC, trampoline, per-CPU descriptor/stack/VMX ownership, entry-code, interrupt, and teardown support.

The scheduler model can still reject invalid pCPU/vCPU mappings before that hardware path exists.

## Test Impact

Unit tests cover the AP startup plan constraints: low-memory trampoline, stack size, stack alignment, and nonzero AP count. The observed QEMU TCG boot is BSP-only and is not AP evidence. Later APIC and trampoline code must add hardware-gated tests or opt-in QEMU evidence that records CPU count, per-CPU entry, descriptor state, and failure handling.
