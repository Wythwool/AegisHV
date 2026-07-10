# Type-1 / VMI / trap backend roadmap

A production Type-1 EDR is a separate runtime, not a larger tracefs parser. AegisHV now has that separation: the default `aegishv` binary remains the Linux host sensor, while `aegishv-type1-kernel` is an opt-in, bootable x86_64 lab target.

The phase backlog lives in `../BACKLOG.md`. That file tracks IDs, scope, acceptance criteria, and production gates. This document records the current low-level boundary and the remaining implementation order. VMI safety and consistency rules live in `VMI.md`.

## Type-1 foundation now present

- A current Limine configuration, page-separated x86_64 ELF layout, kernel builder, ISO builder, and bounded QEMU evidence tooling.
- Validated Limine base revision, HHDM, memory-map, and executable-address handoff with aligned physical relocation support.
- Early transition fault handling and owned GDT, TSS, IDT, double-fault/NMI/machine-check stacks, boot stack, and VM-exit stack state.
- One bounded physical-allocation ledger, retained across VMX preflight and guest setup, for fifteen distinct pages: VMXON, VMCS, ten guest/EPT pages, trap-all I/O A/B pages, and one fixed MSR page. The MSR bitmap permits exactly direct guest `RDMSR IA32_PAT`; all writes and other reads trap. The ledger excludes the linked kernel image and inherited active CR3 root before allocating only from Limine `USABLE` memory below 4 GiB.
- A linker-owned four-page host hierarchy for the final Intel path. After HHDM materialization, it enables NXE/WP, refuses LA57, flushes inherited global translations, and switches to 4K RX/R/RW mappings for only the linked 2 MiB higher-half kernel window; null, HHDM, identity, and five lower stack-guard pages remain absent.
- Intel VMX feature, feature-control, CR0/CR4 fixed-bit, true-control, host-state, `IA32_VMX_MISC` preemption-timer-rate, known-broken timer CPU-signature refusal, and EPT capability validation.
- A complete VMCS, four-level guest paging, four-level EPT, and an assembly VM-entry/exit trampoline for one isolated 64-bit guest.
- A fixed guest with a finite deadline probe followed by the two contained port writes, CPUID, trapped synthetic `IA32_EFER`, direct `IA32_PAT`, `FNOP`, `MOVDQA` self, and HLT. The VMCS loads a deliberate valid guest PAT, saves it on exit, restores the host PAT, and checks both. Under `TS=1`, `EM=0`, and `OSFXSR=1`, the x87 and SIMD probes must each produce an exact `#NM` at its fixed RIP. The host `.text` disassembly gate rejects FPU/SIMD/state-save instructions. These are fixed boundary checks, not general context virtualization.
- A strict evidence contract that requires matching valid pre/post-run SHA-256 image digests, a sixteen-marker chain including owned-host-paging validation, preemption, I/O-A, I/O-B, CPUID, trapped RDMSR, PAT state, x87 `#NM`, SIMD `#NM`, and HLT exits, plus one internally consistent CPU-signature/timer diagnostic set. It rejects changed images, contradictory backends, paging failures, skipped VMX operations, host faults, guest timeouts, unexpected exceptions, and guest entry/exit/resume failures.

## Evidence boundary

A modern Limine ISO has booted locally under QEMU TCG through owned descriptor-table installation and runtime preflight. The available TCG environment did not expose VMX, and WHPX was unavailable. That run proves the boot boundary only; because the CR3 switch is on the final Intel path, it does not prove owned-host-paging activation, VMXON, VMLAUNCH, guest execution, VMRESUME, or VM-exit handling.

Intel guest execution remains unproven until a reviewed nested-VMX or bare-metal run captures matching valid pre/post-run SHA-256 image digests, the complete marker chain, and validated CPU/timer diagnostics described in `TYPE1_BOOT_BOUNDARY.md`. Even successful evidence proves only one BSP, one fixed guest, two trap-all I/O exits, one synthetic RDMSR result, one deliberate PAT transition, two exact fixed-probe `#NM` exits, bounded resumes, and one HLT exit. It does not prove general PAT policy, XSAVE/FXSAVE, host FPU/SIMD preservation or context switching, lazy/multi-vCPU state, WRMSR, general exception injection, or production qualification.

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
- Full architectural context policy beyond the fixed proof: XSAVE/FXSAVE, host FPU/SIMD preservation and context switching, lazy/multi-vCPU state, WRMSR PAT, MTRR/PAT/MMIO policy, SMP/per-CPU PAT, required stateful MSRs, selective/dynamic I/O and MSR bitmaps, general exception injection, and broad exit coverage. The fixed bitmap and PAT/`#NM` stages are not a general device or architectural-state policy.
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
4. Extend the fixed PAT/`#NM` proof into architectural context handling: XSAVE/FXSAVE, host and guest FPU/SIMD preservation, lazy/multi-vCPU state, per-CPU PAT plus MTRR/MMIO policy, stateful MSRs and WRMSR, selective/dynamic bitmaps, general exception injection, and recovery paths.
5. Replace the fixed guest with a bounded general loader and explicit VM/vCPU lifecycle.
6. Add device emulation and an IOMMU-backed DMA isolation policy before any passthrough work.
7. Wire live memory/register reads and then the Stage-2 trap lifecycle into the VMI contracts.
8. Add AMD SVM and ARM64 EL2 runtime paths with architecture-specific evidence gates.
9. Qualify hardware, firmware, boot security, update/rollback, observability, crash handling, and long-duration operations before production wording.
