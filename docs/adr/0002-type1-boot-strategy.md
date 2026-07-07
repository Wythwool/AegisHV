# ADR-0002: Type-1 Target Boot Strategy

## Status

Accepted.

## Context

AegisHV currently ships a Linux userspace sensor. The repository now also has `no_std` crates for core type-1 target boundary models, event ABI, and x86 boot helpers, but it does not ship a bootable hypervisor image.

The first boot path needs firmware memory-map handoff, a controlled kernel image format, a way to capture early serial output, and enough room to add per-CPU state before hardware VMX/SVM work starts.

The options considered are:

- UEFI application: direct firmware services and simple local testing, but more firmware surface remains active during early bring-up.
- Limine: stable boot protocol, memory-map handoff, framebuffer and module conventions, and useful QEMU workflow.
- Multiboot2: common boot metadata, but less aligned with higher-half and modern x86_64 kernel setup.
- Custom loader: maximum control, but it would consume time on loader bugs before the hypervisor core has enough code to justify it.

## Decision

Use Limine as the first boot protocol for the target type-1 runtime path.

The initial runtime model keeps identity and direct-map page-table plans explicit. Early serial logging uses COM1 through an x86 arch crate feature. Firmware memory-map parsing and physical page allocation live in `aegishv-hypervisor-core` so the boot path and tests share the same invariants.

This decision does not make the current binary a type-1 hypervisor. It only fixes the first boot protocol target for the `no_std` runtime path.

## Consequences

The repository may add Limine config, linker scripts, and a boot image crate when the runtime has an entry point. Until then, QEMU boot smoke remains opt-in and requires an explicit boot image path.

The host-side sensor stays the default `aegishv` binary. `cargo run -- ...` continues to target the userspace sensor.

Custom loader work is deferred unless Limine blocks a tested requirement that matters for AegisHV.

## Test Impact

Workspace tests must include the `no_std` crates. The QEMU smoke script must not run in normal CI and must fail clearly when no boot image exists.
