# ARM64 EL2 Lab Boundary

This document records the ARM64 EL2 model code now present in `aegishv-arch-arm64`. The current repository still does not ship a bootable ARM64 type-1 image, an EL2 runtime, or host hardware evidence.

## Implemented Model Pieces

- ARM64 capability decoding for EL2, VHE/nVHE, VMID width, IPA size, granule support, GIC virtualization, SMMU presence, and protected guest visibility.
- EL2 vector table skeleton with 2K alignment and all architectural vector slots.
- 4K Stage-2 mapping plans with R/W/X permissions, memory attributes, and shareability.
- VTCR_EL2 and VTTBR_EL2 value construction with IPA size, VMID width, granule, and root-address validation.
- ESR_EL2/FAR_EL2/HPFAR_EL2 abort decoding that keeps FAR validity and Stage-1 page-table-walk context. The decoder does not treat FAR_EL2 as a guest physical address by itself.
- TLBI plans for global, VMID, and IPA invalidation with DSB/ISB barrier requirements.
- Lab trap handling for HVC, SMC, WFI, WFE, instruction abort, data abort, execute traps, and write traps.
- GIC virtualization planning for GICv2/GICv3 and virtual interrupt bounds.
- Minimal virtual timer state for CNTHCTL_EL2, CNTVOFF_EL2, compare values, and trap handling.

These pieces are library models with tests. They are not wired to privileged EL2 instructions on a live ARM64 host.

## Required Lab Exit Coverage

A minimal ARM64 toy guest lab must cover these exits before it is treated as useful lab evidence:

- HVC;
- WFI;
- WFE;
- instruction abort;
- data abort.

The lab validator rejects missing EL2, unsupported 4K Stage-2 granule, bad vector base, invalid VTCR/VTTBR inputs, or missing exit coverage with typed errors.

## Protected Guest Limits

pKVM, Arm CCA realms, vendor protected guests, and similar mechanisms can block memory and register visibility. AegisHV does not claim introspection for protected guest memory. When these protections are detected in a future live backend, the expected behavior is a typed degraded or unsupported result.

## Opt-In Lab Script

`scripts/arm64-el2-lab-smoke.sh` checks ARM64 host prerequisites and can print a QEMU command for a future lab image. It refuses missing artifacts, missing QEMU, and missing `/dev/kvm` when KVM is required.

Example host check:

```bash
scripts/arm64-el2-lab-smoke.sh --check-host --log-dir /tmp/aegishv-arm64-lab
```

Example plan print:

```bash
AEGISHV_ARM64_BOOT_IMAGE=./target/type1/aegishv-arm64.elf \
scripts/arm64-el2-lab-smoke.sh --print-command --log-dir /tmp/aegishv-arm64-lab
```

This script is opt-in lab plumbing. It does not prove that AegisHV boots as an ARM64 type-1 hypervisor.
