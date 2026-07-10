# Intel VMX Lab Boundary

This document records the Intel VMX code now present in `aegishv-arch-x86` and the wired toy-guest path in `aegishv-type1-kernel`. The repository can build a bootable lab ISO when external Limine/xorriso inputs are supplied, but it does not contain reviewed Intel hardware evidence or a production guest runtime.

## Implemented Model Pieces

- VMX capability parsing for `CPUID.1:ECX`, `IA32_FEATURE_CONTROL`, and the `IA32_VMX_MISC` preemption-timer rate, plus refusal of CPU signatures on Linux KVM's known-broken timer denylist.
- VMXON and VMCS region initialization with revision-id and 4K alignment checks.
- Typed VMX instruction facade for VMXON, VMXOFF, VMPTRLD, VMCLEAR, VMLAUNCH, VMRESUME, VMREAD, and VMWRITE. The default executor returns typed unsupported errors instead of pretending hardware execution happened.
- x86_64 hardware executor wrappers for the VMX instructions, including CF/ZF VMfail status handling after each instruction.
- VMCS lifecycle states: allocated, cleared, loaded, launched, and resumable after a modeled exit.
- VMX runtime sequencing for VMXON, VMCLEAR, VMPTRLD, VMLAUNCH, VMRESUME, and VMXOFF around owned VMXON and VMCS regions.
- Minimal VMCS host and toy 64-bit guest state writers with canonical-address and CR fixed-bit validation.
- VMX control field adjustment from allowed-0 and allowed-1 MSR values.
- Explicit handlers for HLT, CPUID, port I/O, VMX preemption timer, RDMSR, WRMSR, CR access, EPT violation, and Monitor Trap Flag exits.
- EPT mapping plans, EPT violation decoding, VPID validation, and INVEPT/VPID invalidation plans.
- Execute and write trap lifecycle models with temporary write-window reporting and Monitor Trap Flag single-step fallback behavior.
- True-control and EPT capability snapshots, complete minimal host/guest/control VMCS state, four-level guest paging, and four-level EPT materialization.
- Trap-all I/O A/B materialization and a fixed MSR bitmap that permits exactly direct guest `RDMSR IA32_PAT`; every write and other read remains trapped. Physical addresses, bitmap controls, bitmap contents, PAT controls, and guest/host PAT fields have exact readback checks.
- A bare-metal assembly trampoline that treats successful VMLAUNCH/VMRESUME as non-returning, saves all guest GPRs, and dispatches VM exits on a dedicated host stack.
- Exact exception/NMI exit decoding for the fixed vector-7 x87 and SIMD guard stages.

Normal tests do not execute privileged VMX instructions. On a VMX-capable boot, the type-1 kernel performs the checked VMXON/VMCS-load cycle, constructs an isolated guest with a finite TSC-or-count deadline probe and HLT fallback, writes the complete VMCS, and calls the assembly VMLAUNCH path. An initial zero-value VMX preemption timer forces a sentinel exit before the first instruction. The handler then derives a reload from the hard `0x01000000`-TSC-tick budget using the `IA32_VMX_MISC` timer rate and resumes the probe. Only after the nonzero VMX deadline expires does it move guest RIP to a payload containing byte writes to ports `0xe9` and `0x8000`, CPUID leaf/subleaf 0, trapped `RDMSR IA32_EFER`, direct `RDMSR IA32_PAT`, `FNOP`, `MOVDQA xmm0,xmm0`, and HLT.

Both I/O pages are trap-all. The MSR page is fixed/read-allowlisted: only the low-range read bit for `IA32_PAT` is clear, while all MSR writes and other reads trap. The exit handler validates the two port operations in order and returns synthetic zero only for the exact trapped `IA32_EFER` stage. It does not emulate or replay the direct PAT read; that instruction executes in the guest.

The VMCS uses a deliberate valid guest PAT and the captured host PAT. VM entry loads the guest value; VM exit saves it and restores the host value. Exact VMCS readback precedes launch, and every exit checks the saved guest field plus live host `IA32_PAT`. The guest directly reads and compares its PAT before reaching the x87 guard. This proves only the fixed PAT transition on a successful recorded run; it is not WRMSR PAT, per-vCPU PAT, or MTRR/PAT/MMIO policy.

The guest runs with `CR0.TS=1`, `CR0.EM=0`, and `CR4.OSFXSR=1`. The side-effect-minimized `FNOP` and `MOVDQA`-self probes must each cause a valid vector-7 hardware-exception exit at its exact fault RIP with no error code. The handler uses exact continuation RIPs rather than VM-exit instruction length. Separately, the ELF inspection path disassembles host `.text` and rejects FPU/SIMD/state-save instructions. Neither mechanism implements XSAVE/FXSAVE, host SIMD preservation, context switching, lazy FPU, or multi-vCPU FPU state.

After the SIMD `#NM`, the fixed state machine injects one vector-6 hardware exception at VM entry while guest RIP names the fixed `UD2` sentinel. An immutable CPL0 IDT gate enters an integer-only handler that advances the saved RIP and returns with `IRETQ` to the fixed HLT. Success is accepted only with the expected cookie and RIP, restored stack, and clear VM-entry and IDT-vectoring valid bits. Delivery-time exceptions and EPT exits fail closed; this path does not reinject them.

The boot path copies a bounded Limine memory map and allocates fifteen distinct pages only from `USABLE` memory between 1 MiB and 4 GiB: VMXON, VMCS, ten guest/EPT pages, and the three bitmap pages. Bootloader-reclaimable memory is deliberately excluded because it can still contain Limine responses and active bootloader page tables.

After runtime and guest pages are materialized through the HHDM, the final Intel path requires NX and four-level paging, sets EFER.NXE and CR0.WP, fills a linker-owned four-page hierarchy, flushes inherited global translations, and switches CR3 before capturing VMCS host state. The live root maps only the linked 2 MiB higher-half kernel window with 4K supervisor leaves: text RX, rodata/GOT R/NX, writable state/stacks/tables RW/NX. Null, HHDM, identity, and five lower stack-guard pages are absent. CR3 and live table contents are read back before `host-paging-ok` is emitted.

The live path remains BSP-only; preflight still uses Limine mappings, and the owned root has no LA57, dynamic/per-CPU mappings, general physical/MMIO or MTRR/PAT policy, teardown, or recovery. It also lacks stateful general MSR/WRMSR virtualization, selective or mutable bitmap policy, general exception delivery and reinjection, APIC and guest-timer virtualization, scheduler-driven preemption, an independent host watchdog, devices/IOMMU isolation, XSAVE/FXSAVE and FPU/SIMD context management, general guest loading, and hardware qualification. The fixed bitmap, PAT, timer, `#NM`, and vector-6 checks are containment evidence, not those missing runtime facilities. A source build or model test is not VMX execution evidence; use matching valid pre/post-run SHA-256 image digests, the strict seventeen-marker chain, and the CPU/timer diagnostic audit described in `TYPE1_BOOT_BOUNDARY.md` on a reviewed nested-VMX or bare-metal host.

## Required Exit Coverage

A minimal Linux VMX lab run must cover these exits before it is treated as meaningful lab evidence:

- CPUID;
- HLT;
- port I/O;
- VMX preemption timer;
- RDMSR;
- exception/NMI exits, including the fixed `#NM` guards;
- WRMSR;
- CR access;
- EPT violation;
- Monitor Trap Flag.

The fixed toy guest now exercises CPUID, HLT, two port-I/O bitmap exits, the preemption timer, one trapped RDMSR exit, one direct PAT read, exact `#NM` exits for one x87 and one SIMD probe, and one fixed vector-6 VM-entry injection/`IRETQ` round trip. It does not exercise WRMSR, CR access, EPT violation, Monitor Trap Flag, general exceptions, error-code injection, reinjection, IST or privilege transitions, external interrupts/APIC, SMP, guest-OS exception support, or FPU/SIMD context switching; those remain requirements for the broader future Linux lab. The validator does not silently turn model coverage into hardware evidence.

## Opt-In Lab Script

`scripts/vmx-linux-lab-smoke.sh` is a local harness for a future boot image and guest kernel. It refuses missing artifacts, missing QEMU, and missing KVM when KVM is required. It is not part of normal CI.

Example plan print:

```bash
AEGISHV_TYPE1_BOOT_IMAGE=./target/type1/aegishv-type1.elf \
AEGISHV_VMX_LAB_KERNEL=./lab/bzImage \
scripts/vmx-linux-lab-smoke.sh --print-command
```

This command is a harness check. It does not prove that AegisHV boots as a type-1 hypervisor.
