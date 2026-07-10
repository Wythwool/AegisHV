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
aegishv:type1:guest-hlt-exit-ok
aegishv:type1:guest-run-ok
```

The host-paging marker is emitted only after NXE/WP and CR3 readback, live-table validation, and owned descriptor-table reachability succeed. `guest-config-ok` additionally requires both bitmap controls and exact live readback of the three nonzero, aligned, distinct, below-4-GiB bitmap addresses. The preemption marker is emitted only after an initial zero-value sentinel exit is followed by a real nonzero timer expiration from the finite TSC-or-count probe. It therefore proves the deadline fired rather than merely that the timer field was written. If an HLT or timer exit occurs at the exact later fallback RIP reached by either limit, the run emits `aegishv:type1:guest-timeout` and regains control instead of wedging the BSP; other unexpected probe exits remain guest-exit errors. The two I/O markers prove trap-all bitmap A contained `OUT 0xe9, AL` and bitmap B contained `OUT DX, AL` at port `0x8000`. The RDMSR marker proves the high-read MSR quadrant contained `IA32_EFER` and returned synthetic zero. The host performs none of those guest operations. CPUID and HLT prove the other bounded VMRESUME stages. Any contradictory backend, host-table or host-paging failure, runtime failure, skipped VMX operation, `aegishv:type1:guest-timeout`, guest entry/exit/resume error, exception, missing-handoff, or panic marker invalidates the run.

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
