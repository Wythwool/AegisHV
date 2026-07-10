# Type-1 Readiness Gate

This gate prevents the bootable Type-1 lab path from being described as a production hypervisor before its runtime and evidence are complete.

## Required Before An Intel Guest-Entry Lab Milestone

- Bootable image and reviewed linker layout.
- CPU entry path, owned host descriptor tables, early allocator, and host fault handling.
- Intel VMX capability and fixed-bit checks before changing control registers.
- VMXON, loaded VMCS, isolated guest memory, EPT, and a complete host/guest VMCS.
- Controlled guest entry, a handled VM exit, VMRESUME, a second handled exit, and VMXOFF.
- Strict opt-in evidence capture with the complete ordered marker chain and contradictory-marker refusal.
- Captured CPU, firmware, accelerator, QEMU, image digest, command line, and serial log.
- Negative evidence showing unsupported hosts fail or skip clearly.
- A hardware-matrix row moved to `checked` with reviewable nested-VMX or bare-metal evidence.

## Required Before Production Runtime Claims

- Hypervisor-owned host page tables and CR3, enforced W^X mappings, and guard pages.
- SMP startup, per-CPU VMX state, vCPU scheduling, APIC/interrupt routing, timers, and preemption.
- General guest loading and lifecycle management rather than one fixed toy guest.
- Complete architectural context policy, including PAT, XSAVE/FPU state, required MSRs, I/O and MSR bitmaps, interrupt injection, and broader exit handling.
- Stage-2 permission updates and invalidation observed under hostile and concurrent guest workloads.
- Device emulation and an IOMMU-backed DMA boundary, or an explicit and enforced no-passthrough policy.
- Live AMD SVM and ARM64 EL2 paths before either architecture is claimed.
- Panic, watchdog, crash recovery, long-duration hardware soak, fuzzing, and firmware/CPU coverage.
- Security review of boot, memory ownership, isolation, update handling, secure/measured boot, attestation, rollback, and incident response.

## Current Result

The boot boundary is implemented: the repository builds a modern Limine ISO, installs owned GDT/TSS/IDT state, validates the Limine handoff, and reaches host runtime preflight in a local QEMU TCG boot.

The Intel toy-guest path is also present in runtime code. It allocates VMXON, VMCS, guest paging, and EPT pages from Limine `USABLE` memory, writes a complete VMCS, executes VMLAUNCH for a `CPUID; HLT` guest, handles the CPUID exit, performs VMRESUME, handles the HLT exit, and shuts VMX down.

The repository still does not pass the Intel guest-entry evidence gate. The available TCG environment does not expose VMX, and WHPX is unavailable, so the local boot reaches the non-VMX/skip path rather than the required guest marker chain. There is no reviewed nested-VMX or bare-metal log proving that the wired guest path executed.

The repository also does not pass the production gate because the host memory, SMP, interrupt/timer, device/IOMMU, general guest, full architectural context, multi-architecture, soak, and security-lifecycle requirements above remain open.

## Wording Rule

Within this claim boundary, release text may say that the repository contains a bootable x86_64 Type-1 lab kernel, that its Limine and owned-host-table path has booted under QEMU TCG, and that an Intel VMX toy-guest path is wired in code.

It must not say that Intel guest execution has been demonstrated until the complete strict marker chain is captured on a reviewed nested-VMX or bare-metal host. It must not describe the repository, the default userspace sensor, or the lab kernel as a production or general-purpose Type-1 hypervisor.
