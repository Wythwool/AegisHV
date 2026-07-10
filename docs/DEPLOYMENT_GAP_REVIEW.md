# Deployment Gap Review

This review is deliberately strict. It lists gaps that block stronger release claims.

## Host Sensor Gaps

- Live tracefs evidence is opt-in and host dependent.
- Package install tests inspect files but do not install packages as root.
- AppArmor, SELinux, and seccomp checks inspect policy material but do not prove enforcement on every distribution.
- Syslog and journald outputs have bounded tests, but fleet-level log pipeline behavior is outside the repository.
- QMP actions are guarded by stable identity rules, but live libvirt lifecycle integration is not implemented.

## VMI Gaps

- Live guest memory reads are not implemented.
- Live guest register reads are not implemented.
- Real Linux and Windows profile extraction is not implemented.
- Offline fixtures do not prove guest OS coverage.
- Confidential guest modes can block inspection.

## Type-1 Gaps Not Closed

- The modern Limine ISO and x86_64 lab-kernel boot path are present, but there is no retained nested-VMX or bare-metal log proving the wired Intel toy guest or final owned CR3 executed. The observed QEMU TCG run reached owned descriptor tables and preflight without VMX; WHPX was unavailable.
- The lab kernel relies on Limine mappings through handoff/preflight, then the final Intel path installs a bounded owned CR3 with W^X leaves and five guards. It still lacks early/dynamic/per-CPU paging, general physical/MMIO mappings, invalidation, teardown/reclamation, recovery, and hardware execution evidence.
- SMP/AP startup, per-CPU VMX state, vCPU scheduling, APIC/interrupt routing, guest-timer virtualization, scheduler-driven preemption, and interrupt injection are not implemented. The fixed toy guest's VMX preemption timer only enforces per-stage deadlines.
- The live Intel path allocates fifteen distinct pages, including immutable trap-all I/O A/B pages and one fixed MSR bitmap, and requires exact VMCS bitmap-address readback. The MSR bitmap allows exactly direct guest `RDMSR IA32_PAT`; all writes and other reads trap, with only the fixed trapped `IA32_EFER` stage receiving a synthetic response. The payload also checks a deliberate valid guest PAT and reaches exact `#NM` exits for `FNOP` and `MOVDQA` self before HLT. A general guest loader, reusable lifecycle, multiple guests, and guest crash recovery are not implemented.
- The fixed PAT and `#NM` probes do not close XSAVE/FXSAVE, host SIMD preservation or context switching, lazy/multi-vCPU FPU state, WRMSR PAT, MTRR/PAT/MMIO policy, SMP/per-CPU PAT, comprehensive stateful MSR handling, general exception injection, selective/dynamic bitmap policy, broad exit coverage, or hostile-guest recovery.
- General direct EPT/NPT/Stage-2 permission changes, invalidation, and single-step/retrap enforcement are not implemented. The Intel toy guest has only its fixed EPT.
- Device isolation is modeled but not programmed into VT-d, AMD-Vi, or an SMMU; there is no production device model or DMA isolation boundary.
- Live AMD SVM guest entry and ARM64 EL2 guest execution are not implemented.
- Hardware soak, broad CPU/firmware coverage, secure/measured boot, attestation, signed rollback-safe updates, crash evidence, and a supported incident-response lifecycle are absent.

## Release Decision

The default host-side sensor can keep moving through its own release gates. The x86_64 Type-1 boot boundary may be described as a lab milestone, but Intel guest execution remains unproven and production Type-1, VMI alpha, and host-sensor release claims must keep separate evidence and gates.
