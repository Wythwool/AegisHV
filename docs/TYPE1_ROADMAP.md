# Type-1 / VMI / trap backend roadmap

A production Type-1 EDR is a separate runtime, not a larger tracefs parser. AegisHV now has that separation: the default `aegishv` binary remains the Linux host sensor, while `aegishv-type1-kernel` is an opt-in, bootable x86_64 lab target.

The phase backlog lives in `../BACKLOG.md`. That file tracks IDs, scope, acceptance criteria, and production gates. This document records the current low-level boundary and the remaining implementation order. VMI safety and consistency rules live in `VMI.md`.

## Type-1 foundation now present

- A current Limine configuration, page-separated x86_64 ELF layout, kernel builder, ISO builder, and bounded QEMU evidence tooling.
- Validated Limine base revision, HHDM, memory-map, and executable-address handoff with aligned physical relocation support.
- Early transition fault handling and owned GDT, TSS, IDT, double-fault/NMI/machine-check stacks, boot stack, and VM-exit stack state.
- One bounded physical-allocation ledger, retained across VMX preflight and guest setup, for VMXON, VMCS, guest, guest-page-table, and EPT pages. It excludes the linked kernel image and the inherited active CR3 root before allocating only from Limine `USABLE` memory.
- A linker-owned four-page host hierarchy for the final Intel path. After HHDM materialization, it enables NXE/WP, refuses LA57, flushes inherited global translations, and switches to 4K RX/R/RW mappings for only the linked 2 MiB higher-half kernel window; null, HHDM, identity, and five lower stack-guard pages remain absent.
- Intel VMX feature, feature-control, CR0/CR4 fixed-bit, true-control, host-state, `IA32_VMX_MISC` preemption-timer-rate, known-broken timer CPU-signature refusal, and EPT capability validation.
- A complete VMCS, four-level guest paging, four-level EPT, and an assembly VM-entry/exit trampoline for one isolated 64-bit guest.
- A fixed guest with a finite TSC-or-count deadline probe and HLT fallback followed by an `AL='A'; OUT 0xE9,AL; CPUID leaf/subleaf 0; HLT` payload, unconditional I/O exiting, an initial zero-value timer sentinel, and a real nonzero deadline exit from the probe. The reload is derived from a hard `0x01000000`-TSC-tick budget and `IA32_VMX_MISC`, cannot produce an effective deadline above that budget, and is refused below 2. The probe fallback uses an eight-times-later TSC horizon plus a finite iteration limit and reports `guest-timeout` rather than wedging the BSP if the timer does not fire. Only after the VMX deadline exit does the host move RIP to the payload; its I/O is validated without replaying the write, later VMRESUME paths remain bounded, and HLT is followed by VMXOFF.
- A strict evidence contract that requires matching valid pre/post-run SHA-256 image digests, an eleven-marker chain including owned-host-paging validation, preemption, I/O, CPUID, and HLT exits, and one internally consistent CPU-signature/timer diagnostic set; it rejects changed images, contradictory backends, paging failures, skipped VMX operations, host faults, guest timeouts, and guest entry/exit/resume failures.

## Evidence boundary

A modern Limine ISO has booted locally under QEMU TCG through owned descriptor-table installation and runtime preflight. The available TCG environment did not expose VMX, and WHPX was unavailable. That run proves the boot boundary only; because the CR3 switch is on the final Intel path, it does not prove owned-host-paging activation, VMXON, VMLAUNCH, guest execution, VMRESUME, or VM-exit handling.

Intel guest execution remains unproven until a reviewed nested-VMX or bare-metal run captures matching valid pre/post-run SHA-256 image digests, the complete marker chain, and validated CPU/timer diagnostics described in `TYPE1_BOOT_BOUNDARY.md`. Even successful evidence proves only one BSP, one fixed guest, its containment and port-I/O exits, one CPUID exit, bounded resumes, and one HLT exit. It is not production qualification.

## Backend contracts now present

`../src/hypervisor.rs` defines the architecture-neutral shape needed by a future live backend adapter:

- VM identity and vCPU identity;
- VM-exit classification;
- Stage-2 permission updates;
- backend event pump;
- backend health snapshot.

`../src/vmi.rs` defines the semantic layer needed above that adapter:

- guest physical memory reads;
- vCPU register reads;
- guest virtual-to-physical translation;
- OS profile loading;
- process/module/symbol attribution;
- syscall-path reporting;
- trap lifecycle calls.

These userspace contracts are not wired to the bare-metal toy guest. The repository has real Intel bring-up code, but it does not yet have a live VMI backend or a general trap runtime.

## Required subsystems for production

- Owned paging for handoff and preflight rather than only the final Intel path; dynamic mapping, invalidation, per-CPU roots, physical/MMIO cache policy, guard-fault recovery tests, reclamation, and explicit teardown.
- SMP/AP startup, per-CPU VMX state, vCPU scheduling, APIC/interrupt routing, guest-timer virtualization, scheduler-driven preemption, and interrupt injection.
- Full architectural context policy, including PAT, XSAVE/FPU state, required MSRs, selective I/O and MSR bitmaps, and broad exit coverage. Unconditional I/O exiting for the fixed guest is not a general device policy.
- A general guest/module loader, reusable VM/vCPU lifecycle, multiple address spaces, memory reclamation, and guest crash recovery.
- A runtime Stage-2 permission manager with huge-page splits, invalidation, single-step/retrap, storm control, concurrency tests, and hostile-guest coverage.
- Device emulation plus an IOMMU-enforced DMA boundary for any passthrough path. Page ownership, DMA-domain, PCI, VT-d/AMD-Vi/SMMU, and quarantine models are not hardware programming.
- A live guest memory/register adapter and consistent snapshot/retry rules before the offline VMI layer can inspect a running guest.
- Linux and Windows profile distribution, symbol resolution, process/module attribution, and syscall-path checks on live data.
- Live AMD SVM and ARM64 EL2 entry paths before those architectures are claimed.
- An independent host watchdog for timer failure, panic/watchdog recovery, crash evidence, long-duration hardware soak, broad CPU/firmware coverage beyond the fixed known-broken signature denylist, fuzzing, secure/measured boot, attestation, signed rollback-safe updates, and incident response.

## Next implementation sequence

1. Capture the complete Intel toy-guest chain on a reviewed nested-VMX or bare-metal host and fix any VM-instruction error without weakening the evidence contract.
2. Extend the bounded final-path root into a complete early-boot/per-CPU paging policy with dynamic mappings, invalidation, teardown, and recoverable guard-fault tests.
3. Add AP startup, per-CPU host/VMX state, APIC interrupts, timers, and a minimal preemptible vCPU scheduler.
4. Complete architectural context handling: PAT, XSAVE/FPU, MSRs, bitmaps, interrupt injection, and recovery paths.
5. Replace the fixed guest with a bounded general loader and explicit VM/vCPU lifecycle.
6. Add device emulation and an IOMMU-backed DMA isolation policy before any passthrough work.
7. Wire live memory/register reads and then the Stage-2 trap lifecycle into the VMI contracts.
8. Add AMD SVM and ARM64 EL2 runtime paths with architecture-specific evidence gates.
9. Qualify hardware, firmware, boot security, update/rollback, observability, crash handling, and long-duration operations before production wording.
