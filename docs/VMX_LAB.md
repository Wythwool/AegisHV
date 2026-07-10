# Intel VMX Lab Boundary

This document records the Intel VMX code now present in `aegishv-arch-x86` and the wired toy-guest path in `aegishv-type1-kernel`. The repository can build a bootable lab ISO when external Limine/xorriso inputs are supplied, but it does not contain reviewed Intel hardware evidence or a production guest runtime.

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
- True-control and EPT capability snapshots, complete minimal host/guest/control VMCS state, four-level guest paging, and four-level EPT materialization.
- A bare-metal assembly trampoline that treats successful VMLAUNCH/VMRESUME as non-returning, saves all guest GPRs, and dispatches VM exits on a dedicated host stack.

Normal tests do not execute privileged VMX instructions. On a VMX-capable boot, the type-1 kernel performs the checked VMXON/VMCS-load cycle, constructs an isolated eight-byte guest (`mov eax, 0; cpuid; hlt`), writes the complete VMCS, and calls the assembly VMLAUNCH path. The CPUID exit proves initial entry; the subsequent HLT exit proves VMRESUME. Any other exit or VM-entry/resume failure is terminal and emits a failure marker.

The boot path copies a bounded Limine memory map and allocates all twelve VMX and toy-guest pages only from `USABLE` memory between 1 MiB and 4 GiB. Bootloader-reclaimable memory is deliberately excluded because it can still contain Limine responses and active bootloader page tables.

The live path remains BSP-only and lacks interrupts, timers, devices, XSAVE state, general guest loading, and hardware qualification. A source build or model test is not VMX execution evidence; use the strict full marker chain described in `TYPE1_BOOT_BOUNDARY.md` on a reviewed nested-VMX or bare-metal host.

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
