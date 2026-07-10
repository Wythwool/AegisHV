# ADR-0002: Type-1 Target Boot Strategy

## Status

Accepted.

## Context

AegisHV ships a Linux userspace sensor and now also contains a bootable x86_64 Type-1 lab kernel. The lab kernel uses the `no_std` core, event ABI, x86 boot, VMX, device-model, and boot-handoff crates in a separate target path; it is not the default `aegishv` binary.

The first boot path needed firmware memory-map handoff, a controlled kernel image format, deterministic early serial output, and enough control over host architectural state to bring up a minimal isolated guest.

The options considered were:

- UEFI application: direct firmware services and simple local testing, but more firmware surface remains active during early bring-up.
- Limine: stable boot protocol, memory-map handoff, framebuffer and module conventions, and a useful QEMU workflow.
- Multiboot2: common boot metadata, but less aligned with higher-half and modern x86_64 kernel setup.
- Custom loader: maximum control, but it would consume time on loader bugs before the hypervisor core justified that cost.

## Decision

Use Limine as the first boot protocol for the target Type-1 runtime path.

The runtime keeps physical ownership and direct-map assumptions explicit. Early serial logging uses COM1, Limine supplies the HHDM and usable memory map, and the kernel installs owned GDT, TSS, IDT, and VM-exit state before Intel VMX bring-up. Firmware memory-map parsing and physical page allocation remain in `aegishv-hypervisor-core` so the boot path and tests share the same invariants.

This decision fixes the boot protocol and handoff boundary. It does not make the default userspace binary a hypervisor, and it does not by itself qualify the lab kernel for production.

## Consequences

The repository has current Limine configuration, a page-separated linker layout, a boot handoff crate, a kernel ELF builder, ISO staging and building helpers, and strict QEMU evidence tooling. With reviewed Limine and xorriso inputs, these artifacts produce a bootable ISO rather than only an image plan.

A local QEMU TCG run has booted that ISO through the owned host-table and runtime-preflight boundary. TCG does not expose VMX in the available environment, and WHPX is unavailable, so this observation does not prove VMXON, VMLAUNCH, VMRESUME, or guest exit handling.

The Intel path nevertheless wires a fixed `CPUID; HLT` guest, guest paging, EPT, complete VMCS setup, an assembly entry/exit trampoline, CPUID exit handling, VMRESUME, HLT exit handling, and VMX shutdown. It requires a reviewed nested-VMX or bare-metal run with the complete strict marker chain before guest execution is claimed.

The host-side sensor stays the default `aegishv` binary. `cargo run -- ...` continues to target the userspace sensor.

Custom loader work remains deferred unless Limine blocks a tested requirement that matters for AegisHV. A future production path must still replace inherited mappings with a hypervisor-owned CR3 and enforced W^X/guard pages, then add SMP/per-CPU state, interrupts/timers, devices and IOMMU isolation, a general guest loader, full architectural context, multi-architecture runtime coverage, and a security/operations lifecycle.

## Test Impact

Workspace tests include the `no_std` crates and bare-metal kernel build checks. QEMU boot remains opt-in and is not part of normal CI. The smoke path accepts a bootable Limine ISO, uses a bounded timeout, and refuses to treat the run as Intel evidence unless the full host-table, VMX entry, CPUID exit, VMRESUME, HLT exit, and completion chain appears in order without a forbidden marker.
