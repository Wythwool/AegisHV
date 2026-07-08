# Type-1 Readiness Gate

This gate prevents a planned type-1 path from being described as implemented too early.

## Required Before A Lab Milestone

- Bootable image and linker layout.
- CPU entry path for at least one architecture.
- Serial marker evidence from an opt-in QEMU boot path.
- Early allocator and per-CPU state.
- VM creation and vCPU entry path.
- Controlled guest exit and shutdown path.
- Crash record path that survives the lab run.
- Opt-in QEMU script with captured logs.
- Hardware matrix row moved to checked with evidence.

## Required Before Runtime Claims

- Intel VMX, AMD SVM, or ARM64 EL2 backend code, not only models.
- Stage-2 permission update path that can be observed in a guest.
- TLB invalidation path with negative tests.
- Guest memory/register read path with typed errors.
- Device isolation backend or an explicit no-passthrough policy.
- Panic, watchdog, and crash recovery behavior.
- Security review of boot, memory ownership, device isolation, and update handling.

## Current Result

The current repository does not pass this gate. It has useful no-std model crates, lab model tests, device isolation models, opt-in scripts, a planned boot skeleton, a minimal type-1 kernel ELF build path with a Limine request block, local ELF inspection, ISO-root staging, and a type-1 image-plan manifest, but no bootable type-1 runtime.

## Wording Rule

Release text may say that the repository contains planned type-1 boundary models and lab scaffolding. It must not say that the current binary is a type-1 hypervisor.
