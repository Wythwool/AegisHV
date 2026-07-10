# Type-1 Lab Milestone Gate

The x86_64 boot-boundary milestone is implemented: a modern Limine ISO boots locally under QEMU TCG, installs the owned GDT/TSS/IDT, accepts the validated handoff, and reaches runtime preflight. The Intel guest-entry milestone remains blocked until the [Type-1 readiness gate](TYPE1_READINESS_GATE.md) passes.

The VMX toy-guest path is wired in code, but the available TCG environment does not provide VMX and WHPX is unavailable. The observed TCG run therefore reaches the non-VMX and skipped-operation markers; it is boot evidence, not guest-execution evidence.

## Required Intel Evidence

A successful Intel lab run must contain this complete ordered serial chain:

```text
aegishv:type1:host-tables-ok
aegishv:type1:backend-vmx
aegishv:type1:vmxon-cycle-ok
aegishv:type1:vmcs-load-ok
aegishv:type1:guest-config-ok
aegishv:type1:guest-cpuid-exit-ok
aegishv:type1:guest-hlt-exit-ok
aegishv:type1:guest-run-ok
```

The CPUID exit proves initial VMLAUNCH entry. The HLT exit after it proves that VMRESUME returned to the guest. Any contradictory backend, host-table failure, runtime failure, skipped VMX operation, guest entry/exit/resume error, exception, missing-handoff, or panic marker invalidates the run.

The [Type-1 boot boundary](TYPE1_BOOT_BOUNDARY.md) defines what this chain proves and what remains outside it.

## Candidate Evidence Package

- boot image path and checksum;
- boot boundary manifest from `scripts/build-type1-skeleton.sh`;
- kernel ELF build manifest from `scripts/build-type1-kernel.sh`;
- kernel ELF inspection manifest from `scripts/inspect-type1-kernel.sh`;
- ISO-root staging manifest from `scripts/stage-type1-limine-iso.sh`;
- Limine ISO build manifest from `scripts/build-type1-limine-iso.sh`;
- local tool manifest from `scripts/check-type1-lab-tools.sh`;
- image input manifest from `scripts/plan-type1-image.sh`;
- QEMU command line and image digest;
- serial log containing the complete ordered chain above and no forbidden marker;
- QEMU smoke evidence manifest from `scripts/type1-qemu-evidence.sh`;
- local lab-chain summary from `scripts/run-type1-lab.sh`;
- host CPU, firmware, accelerator, and QEMU versions;
- VM-instruction error output for any failed entry or resume attempt;
- shutdown or crash record;
- negative test showing unsupported hosts fail or skip clearly.

## Release Notes Shape

Within the current claim boundary, release text may describe a bootable Type-1 lab kernel and a local TCG boot through owned host tables and preflight. It may describe the Intel toy-guest path as implemented in code.

Use "Intel guest-entry lab milestone" wording only after the complete evidence package exists. Do not claim demonstrated VMX guest execution from the current TCG run, and do not describe the host-side binary or lab kernel as a production hypervisor.
