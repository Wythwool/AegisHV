# Threat model

AegisHV has two separate security surfaces with different claims.

- The default `aegishv` binary is a Linux host-side KVM telemetry sensor. It trusts the host kernel, tracefs, process metadata, configured QMP sockets, policy files, and the operator account.
- `aegishv-type1-kernel` is a bootable x86_64 bring-up target for one fixed Intel VMX toy guest. It is lab code, not a production isolation boundary.

## Good fit

- KVM hosts where tracefs is available and the operator accepts host-side sensor limits.
- Detection pipelines that want host-side signals without guest agents.
- W^X-like correlation, suspicious exit activity, and policy-driven QMP response.
- Reviewed lab machines used to validate the Type-1 boundary: the Limine boot path, host architectural state, and the fixed VMX guest-entry/exit sequence.

## Weak fit

- Guests protected by technologies that intentionally hide guest memory or register state.
- Environments that need per-process or per-module attribution without a live VMI layer.
- Claims of full syscall-path integrity without consistent guest memory, registers, and symbols.
- Multi-tenant or hostile-guest deployments that require a production-qualified hypervisor security boundary.

## Type-1 evidence boundary

The modern Limine ISO has booted locally under QEMU TCG through owned GDT/TSS/IDT installation and runtime preflight. TCG did not expose VMX in the available environment, and WHPX was unavailable. This is boot evidence only.

The source contains VMXON, complete VMCS and EPT setup, VMLAUNCH into a finite TSC-or-count deadline probe with an HLT fallback, a zero-value timer sentinel followed by a real nonzero deadline exit, and a fixed `AL='A'; OUT 0xE9,AL; CPUID leaf/subleaf 0; HLT` payload. If the timer fails, either the TSC horizon or finite iteration limit reaches the fallback and returns control through a `guest-timeout` path. Unconditional I/O exiting, validated and suppressed port-I/O handling, bounded resumes through CPUID and HLT, and VMXOFF follow. Those paths are not treated as executed until matching valid pre/post-run SHA-256 image digests, the complete strict marker chain, and the CPU/timer diagnostic audit are captured on a reviewed nested-VMX or bare-metal host.

## Threats not yet contained by the lab kernel

- Malicious or malformed general guests; only the fixed deadline probe and payload are in scope.
- SMP races, cross-CPU VMX state, APIC/interrupt/guest-timer attacks, live scheduling, and scheduler-driven preemption. The fixed path's VMX preemption timer is only a stage deadline.
- Host page-table alias attacks before the final Intel switch and outside its fixed 2 MiB window. The final path removes HHDM/identity aliases and validates W^X plus five guards, but early/dynamic/per-CPU paging, teardown, recovery, and hardware qualification remain absent.
- FPU/XSAVE, PAT, MSR, selective I/O/device policy, interrupt-injection, and broad VM-exit state confusion. Unconditional I/O exiting contains the fixed guest's port access but is not a general device model.
- DMA and malicious devices; no IOMMU-backed isolation boundary or production device model is live.
- Guest-loader, image-parser, multi-VM, overcommit, migration, suspend/resume, or guest crash-recovery threats.
- AMD SVM and ARM64 EL2 runtime threats; those live paths are absent.
- Secure/measured boot, rollback-safe updates, attestation, secrets handling, persistent crash evidence, long-duration fault containment, and operational response.

## Honest limits

Describe the default binary as a hardened KVM host-side sensor. Describe the separate no-std target as a bootable Type-1 lab kernel with an Intel toy-guest path implemented in code.

Do not describe the TCG boot as VMX execution evidence, and do not describe either target as a finished VMX/SVM/EL2 EDR hypervisor, a full VMI stack, or a production isolation boundary.
