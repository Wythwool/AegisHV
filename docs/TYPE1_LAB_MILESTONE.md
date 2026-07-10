# Type-1 Lab Milestone Gate

The x86_64 boot-boundary milestone is implemented: a modern Limine ISO boots locally under QEMU TCG, installs the owned GDT/TSS/IDT, accepts the validated handoff, and reaches runtime preflight. The Intel guest-entry milestone remains blocked until the [Type-1 readiness gate](TYPE1_READINESS_GATE.md) passes.

The VMX toy-guest path is wired in code, but the available TCG environment does not provide VMX and WHPX is unavailable. The observed TCG run therefore stops before the final owned-CR3 path and reaches the non-VMX/skipped-operation markers; it is boot evidence, not owned-paging or guest-execution evidence.

## Required Intel Evidence

A successful Intel lab run must contain this complete ordered serial chain:

```text
aegishv:type1:host-tables-ok
aegishv:type1:backend-vmx
aegishv:type1:vmxon-cycle-ok
aegishv:type1:vmcs-load-ok
aegishv:type1:host-paging-ok
aegishv:type1:guest-config-ok
aegishv:type1:guest-preempt-exit-ok
aegishv:type1:guest-io-exit-ok
aegishv:type1:guest-io-b-exit-ok
aegishv:type1:guest-cpuid-exit-ok
aegishv:type1:guest-rdmsr-exit-ok
aegishv:type1:guest-pat-state-ok
aegishv:type1:guest-nm-x87-exit-ok
aegishv:type1:guest-nm-simd-exit-ok
aegishv:type1:guest-hlt-exit-ok
aegishv:type1:guest-run-ok
```

The host-paging marker is emitted only after NXE/WP and CR3 readback, live-table validation, and owned descriptor-table reachability succeed. `guest-config-ok` additionally requires both bitmap controls, exact bitmap-address readback, PAT control support, exact guest/host PAT-field readback, and the fixed `TS=1`, `EM=0`, `OSFXSR=1` state. The preemption marker requires a real nonzero timer expiration after the zero sentinel. The two I/O markers prove the trap-all pages contained the expected port operations, while the RDMSR marker proves the trapped `IA32_EFER` stage returned synthetic zero. The fixed MSR page permits only direct guest `RDMSR IA32_PAT`; all writes and other reads remain trapped.

`guest-pat-state-ok` requires the direct PAT read to match the deliberate valid guest value, the saved guest VMCS field to match, and the captured host PAT to be restored and read back. `guest-nm-x87-exit-ok` and `guest-nm-simd-exit-ok` require exact vector-7 hardware-exception exits at the fixed `FNOP` and `MOVDQA`-self RIPs. Those instructions minimize side effects if a guard regresses, but success still requires that neither executes. These three markers make the chain sixteen entries long; they do not prove XSAVE/FXSAVE, host SIMD preservation, context switching, or general exception injection. Any contradictory backend, host-table or host-paging failure, runtime failure, skipped VMX operation, `guest-timeout`, guest entry/exit/resume error, unexpected exception, missing handoff, or panic invalidates the run.

The [Type-1 boot boundary](TYPE1_BOOT_BOUNDARY.md) defines what this chain proves and what remains outside it.

## Candidate Evidence Package

- boot image path;
- boot boundary manifest from `scripts/build-type1-skeleton.sh`;
- kernel ELF build manifest from `scripts/build-type1-kernel.sh`;
- kernel ELF inspection manifest from `scripts/inspect-type1-kernel.sh`;
- ISO-root staging manifest from `scripts/stage-type1-limine-iso.sh`;
- Limine ISO build manifest from `scripts/build-type1-limine-iso.sh`;
- local tool manifest from `scripts/check-type1-lab-tools.sh`;
- image input manifest from `scripts/plan-type1-image.sh`;
- QEMU command line plus matching valid pre/post-run SHA-256 image digests;
- serial log containing the complete ordered chain above and no forbidden marker;
- QEMU smoke evidence manifest from `scripts/type1-qemu-evidence.sh`;
- local lab-chain summary from `scripts/run-type1-lab.sh`;
- host CPU, firmware, accelerator, and QEMU versions;
- exactly one serial-reported CPUID signature and internally consistent VMX timer rate, reload, and effective-deadline diagnostic set;
- VM-instruction error output for any failed entry or resume attempt;
- shutdown or crash record;
- negative test showing unsupported hosts fail or skip clearly.

## Release Notes Shape

Within the current claim boundary, release text may describe a bootable Type-1 lab kernel and a local TCG boot through owned descriptor tables and preflight. It may describe the Intel toy-guest and final owned-CR3 path as implemented in code, not observed under TCG.

Use "Intel guest-entry lab milestone" wording only after the complete evidence package exists. Do not claim demonstrated VMX guest execution from the current TCG run, and do not describe the host-side binary or lab kernel as a production hypervisor.
