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

A local QEMU TCG run has booted that ISO through the owned descriptor-table and runtime-preflight boundary. TCG does not expose VMX in the available environment, and WHPX is unavailable, so this observation stops before the final owned-CR3 path and does not prove its activation, VMXON, VMLAUNCH, VMRESUME, or guest exit handling.

The Intel path nevertheless wires a finite TSC-or-count deadline probe with an HLT fallback and a fixed payload containing byte writes to ports `0xe9` and `0x8000`, CPUID leaf/subleaf 0, `RDMSR IA32_EFER`, and HLT, plus guest paging, EPT, complete VMCS setup, and an assembly entry/exit trampoline. One allocator owns fifteen distinct pages, including trap-all I/O A/B/MSR pages. The VMCS requires both bitmap controls and exact live address readback. A real nonzero deadline follows the zero-value sentinel before the host moves RIP to the payload. The handlers suppress both port operations, return synthetic zero for RDMSR, never perform guest I/O or MSR on the host, keep every resume bounded, and shut VMX down. A reviewed nested-VMX or bare-metal run with matching valid pre/post-run SHA-256 image digests, the complete strict marker chain, and validated CPU/timer diagnostics is still required before guest execution is claimed.

The host-side sensor stays the default `aegishv` binary. `cargo run -- ...` continues to target the userspace sensor.

Custom loader work remains deferred unless Limine blocks a tested requirement that matters for AegisHV. The final Intel path now replaces inherited aliases with a bounded four-level CR3 and W^X/guard layout, but a production path must extend that policy across early boot, dynamic/per-CPU mappings, invalidation, teardown, and recovery, then add SMP/per-CPU state, interrupt and guest-timer virtualization, scheduler-driven preemption, devices and IOMMU isolation, a general guest loader, full architectural context, multi-architecture runtime coverage, and a security/operations lifecycle.

## Test Impact

Workspace tests include the `no_std` crates and bare-metal kernel build checks. QEMU boot remains opt-in and is not part of normal CI. The evidence path requires matching valid pre/post-run SHA-256 image digests and refuses to treat the run as Intel evidence unless the full thirteen-marker host-table, VMX-backend/VMXON/VMCS-load, owned-paging, guest-configuration/preemption/I/O-A/I/O-B/CPUID/RDMSR/HLT/completion chain appears in order without a forbidden marker.
