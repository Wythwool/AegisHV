# Intel VMX Lab Boundary

This document records the Intel VMX model code now present in `aegishv-arch-x86`. The current repository still does not ship a bootable type-1 image, a wired guest run, or host hardware evidence.

## Implemented Model Pieces

- VMX capability parsing for `CPUID.1:ECX` and `IA32_FEATURE_CONTROL`.
- VMXON and VMCS region initialization with revision-id and 4K alignment checks.
- Typed VMX instruction facade for VMXON, VMXOFF, VMPTRLD, VMCLEAR, VMLAUNCH, VMRESUME, VMREAD, and VMWRITE. The default executor returns typed unsupported errors instead of pretending hardware execution happened.
- x86_64 hardware executor wrappers for the VMX instructions, including CF/ZF VMfail status handling after each instruction.
- VMCS lifecycle states: allocated, cleared, loaded, launched, and resumable after a modeled exit.
- VMX runtime sequencing for VMXON, VMCLEAR, VMPTRLD, VMLAUNCH, VMRESUME, and VMXOFF around owned VMXON and VMCS regions.
- Minimal VMCS host and toy 64-bit guest state writers with canonical-address and CR fixed-bit validation.
- VMX control field adjustment from allowed-0 and allowed-1 MSR values.
- Explicit handlers for HLT, CPUID, RDMSR, WRMSR, CR access, EPT violation, and Monitor Trap Flag exits.
- EPT mapping plans, EPT violation decoding, VPID validation, and INVEPT/VPID invalidation plans.
- Execute and write trap lifecycle models with temporary write-window reporting and Monitor Trap Flag single-step fallback behavior.

These pieces are library code with tests. Normal tests do not execute privileged VMX instructions, and the hardware executor is not wired into the type-1 boot image or hardware evidence path yet.

## Required Exit Coverage

A minimal Linux VMX lab run must cover these exits before it is treated as meaningful lab evidence:

- CPUID;
- HLT;
- RDMSR;
- WRMSR;
- CR access;
- EPT violation;
- Monitor Trap Flag.

The lab validator rejects missing VMX, EPT, VPID, or missing exit coverage with typed errors. It does not silently pass a partial run.

## Opt-In Lab Script

`scripts/vmx-linux-lab-smoke.sh` is a local harness for a future boot image and guest kernel. It refuses missing artifacts, missing QEMU, and missing KVM when KVM is required. It is not part of normal CI.

Example plan print:

```bash
AEGISHV_TYPE1_BOOT_IMAGE=./target/type1/aegishv-type1.elf \
AEGISHV_VMX_LAB_KERNEL=./lab/bzImage \
scripts/vmx-linux-lab-smoke.sh --print-command
```

This command is a harness check. It does not prove that AegisHV boots as a type-1 hypervisor.
