# Type-1 Readiness Gate

This gate prevents the bootable Type-1 lab path from being described as a production hypervisor before its runtime and evidence are complete.

## Required Before An Intel Guest-Entry Lab Milestone

- Bootable image and reviewed linker layout.
- CPU entry path, owned host descriptor tables, early allocator, and host fault handling.
- Intel VMX capability and fixed-bit checks, including refusal of CPU signatures known to have broken VMX preemption timers, before changing control registers.
- VMXON, loaded VMCS, isolated guest memory, EPT, and a complete host/guest VMCS.
- One retained allocator ledger for fifteen distinct VMXON/VMCS/guest/EPT/bitmap pages, trap-all I/O materialization, a fixed MSR bitmap allowing exactly direct `RDMSR IA32_PAT`, both bitmap controls, strict bitmap-address validation, and exact live VMCS address readback.
- Final-path owned CR3 activation with NXE/WP readback, live 4K RX/R/RW table validation, no HHDM/identity alias, and five non-present stack guards.
- Controlled guest entry, a zero-value VMX preemption-timer sentinel followed by a proven nonzero deadline exit from a finite TSC-or-count probe with an HLT timeout fallback, validated and suppressed exits through I/O bitmap A and B, CPUID, a synthetic high-read RDMSR result, deliberate guest PAT load/read/save plus host restore, exact `#NM` exits for fixed `FNOP` and `MOVDQA`-self probes, and HLT separated by bounded VMRESUME operations, followed by VMXOFF.
- Strict opt-in evidence capture with matching valid pre/post-run SHA-256 image digests, the complete ordered marker chain, contradictory-marker refusal, and one validated CPU-signature/timer diagnostic set.
- Captured CPU, firmware, accelerator, QEMU, both image digests and their equality result, command line, serial log, and validated CPU-signature/timer-rate/reload/effective-deadline diagnostics.
- Negative evidence showing unsupported hosts fail or skip clearly.
- A hardware-matrix row moved to `checked` with reviewable nested-VMX or bare-metal evidence.

## Required Before Production Runtime Claims

- Owned paging from early handoff onward, including dynamic/per-CPU roots, invalidation, physical/MMIO policy, teardown/reclamation, and recoverable guard-fault coverage rather than only the fixed final Intel path.
- SMP startup, per-CPU VMX state, vCPU scheduling, APIC/interrupt routing, guest-timer virtualization, and scheduler-driven preemption.
- General guest loading and lifecycle management rather than one fixed toy guest.
- Complete architectural context policy beyond the fixed probes, including XSAVE/FXSAVE, host FPU/SIMD preservation and context switching, lazy/multi-vCPU state, WRMSR PAT, MTRR/PAT/MMIO policy, SMP/per-CPU PAT, full required MSR state, selective and dynamic I/O/MSR bitmap policy, general exception injection, and broader exit handling.
- Stage-2 permission updates and invalidation observed under hostile and concurrent guest workloads.
- Device emulation and an IOMMU-backed DMA boundary, or an explicit and enforced no-passthrough policy.
- Live AMD SVM and ARM64 EL2 paths before either architecture is claimed.
- An independent host watchdog, panic and crash recovery, long-duration hardware soak, fuzzing, and firmware/CPU coverage beyond the fixed known-broken timer signature denylist.
- Security review of boot, memory ownership, isolation, update handling, secure/measured boot, attestation, rollback, and incident response.

## Current Result

The boot boundary is implemented: the repository builds a modern Limine ISO, installs owned GDT/TSS/IDT state, validates the Limine handoff, and reaches host runtime preflight in a local QEMU TCG boot.

The Intel toy-guest path is also present in runtime code. One early allocation ledger excludes the complete linked kernel image and inherited active CR3 root before allocating fifteen distinct Limine `USABLE` pages below 4 GiB: VMXON, VMCS, ten guest/EPT pages, trap-all I/O A/B pages, and one fixed MSR page. Invalid bounds, aliases, a changed inherited root, or a bitmap pattern other than the exact one-read PAT allowlist fail closed. The VMCS requires both bitmap controls, validates the three addresses, and requires exact live bitmap and PAT-field readback. The final path then enables NXE/WP, rejects LA57, switches to the four-page owned hierarchy, and validates the live RX/R/RW leaves and five guards before guest entry.

The fixed guest enters a finite TSC-or-count deadline probe with an HLT fallback. A zero-value sentinel precedes a nonzero timer deadline derived from the hard `0x01000000`-TSC-tick budget; reloads below 2 are refused. After that deadline the guest executes the two contained port operations, CPUID, trapped `RDMSR IA32_EFER`, direct `RDMSR IA32_PAT`, `FNOP`, `MOVDQA xmm0,xmm0`, and HLT. The deliberate valid guest PAT is loaded on entry, saved on exit, compared by the guest, and separated from the restored host PAT. The two probe instructions must produce exact `#NM` exits under `TS=1`, `EM=0`, and `OSFXSR=1`. The host `.text` disassembly gate rejects FPU/SIMD/state-save instructions, but no general FPU/SIMD context management is claimed.

The repository still does not pass the Intel guest-entry evidence gate. The available TCG environment does not expose VMX, and WHPX is unavailable, so the local boot reaches the non-VMX/skip path before the owned-root switch rather than the required digest-stable sixteen-marker guest evidence package. There is no reviewed nested-VMX or bare-metal log proving that the wired guest, bitmap exits, PAT transition, `#NM` guards, or final-path CR3 executed.

The repository also does not pass the production gate because the host memory, SMP, interrupt/timer, device/IOMMU, general guest, full architectural context, multi-architecture, soak, and security-lifecycle requirements above remain open.

## Wording Rule

Within this claim boundary, release text may say that the repository contains a bootable x86_64 Type-1 lab kernel, that its Limine and owned descriptor-table path has booted under QEMU TCG, and that an Intel VMX toy-guest plus final-path owned-CR3 transition are wired in code.

It must not say that Intel guest execution has been demonstrated until matching valid pre/post-run SHA-256 image digests, the complete strict marker chain, and validated CPU/timer diagnostics are captured on a reviewed nested-VMX or bare-metal host. It must not describe the repository, the default userspace sensor, or the lab kernel as a production or general-purpose Type-1 hypervisor.
