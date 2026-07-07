# AMD SVM Lab Boundary

This document records the AMD SVM model code now present in `aegishv-arch-x86`. The current repository still does not ship a bootable type-1 image, an AMD SVM root runtime, or host hardware evidence.

## Implemented Model Pieces

- SVM capability parsing for `CPUID.80000001H` and `CPUID.8000000AH`, including SVM, NPT, ASID capacity, flush-by-ASID, decode assists, and AVIC flags.
- EFER.SVME handling as a typed value model.
- 4K-aligned VMCB control and state-save structures with layout tests.
- Typed SVM instruction facade for EFER.SVME, VMRUN, VMLOAD, VMSAVE, and INVLPGA. The default executor returns typed unsupported errors.
- Explicit intercept handlers for CPUID, MSR, CR, IO, HLT, and PAUSE.
- NPT mapping plans with permission updates and protected hypervisor memory ranges.
- Nested page fault decoding and routing to the permission-trap model.
- ASID allocation, release, and INVLPGA invalidation plans.
- Execute and write trap lifecycle models using NPT permissions.
- Tiny SVM guest lab validation that requires explicit intercept coverage before a mock VMRUN path is accepted.

These pieces are library models with tests. They are not wired to privileged SVM instructions on a live host.

## Required Intercept Coverage

A tiny AMD SVM lab run must cover these intercept paths before it is treated as useful lab evidence:

- CPUID;
- MSR;
- CR read or write;
- IO;
- HLT;
- PAUSE;
- nested page fault.

The lab validator rejects missing SVM, NPT, ASID capacity, invalid VMCB addresses, or missing intercept coverage with typed errors.

## SEV Family Limits

SEV, SEV-ES, and SEV-SNP can block or degrade memory and register visibility. AegisHV does not claim a bypass for encrypted guest state.

- SEV can make guest memory contents unavailable to the host-side inspection path.
- SEV-ES can make register state unavailable or require guest cooperation.
- SEV-SNP adds integrity and isolation checks that must be treated as a boundary, not as a feature the current code bypasses.

When these protections are detected in a future live backend, the expected behavior is a typed degraded or unsupported result. Silent success is not acceptable.

## Opt-In Lab Script

`scripts/svm-amd-lab-smoke.sh` checks AMD host prerequisites and can print a QEMU command for a future lab image. It refuses missing artifacts, missing QEMU, missing `/dev/kvm` when required, and missing CPU `svm` flags.

Example host check:

```bash
scripts/svm-amd-lab-smoke.sh --check-host --log-dir /tmp/aegishv-amd-lab
```

Example plan print:

```bash
AEGISHV_TYPE1_BOOT_IMAGE=./target/type1/aegishv-type1.elf \
AEGISHV_SVM_LAB_KERNEL=./lab/bzImage \
scripts/svm-amd-lab-smoke.sh --print-command --log-dir /tmp/aegishv-amd-lab
```

This script is opt-in lab plumbing. It does not prove that AegisHV boots as a type-1 hypervisor.
