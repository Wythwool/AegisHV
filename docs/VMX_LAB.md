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
- A bare-metal assembly trampoline that treats successful VMLAUNCH/VMRESUME as non-returning, saves all guest GPRs, and dispatches VM exits on a dedicated host stack.

Normal tests do not execute privileged VMX instructions. On a VMX-capable boot, the type-1 kernel performs the checked VMXON/VMCS-load cycle, constructs an isolated guest with a finite TSC-or-count deadline probe and HLT fallback followed by the `AL='A'; OUT 0xE9,AL; CPUID leaf/subleaf 0; HLT` payload, writes the complete VMCS, and calls the assembly VMLAUNCH path. An initial zero-value VMX preemption timer forces a sentinel exit before the first instruction. The handler then derives a reload from a hard `0x01000000`-TSC-tick budget using the `IA32_VMX_MISC` timer rate and resumes the probe. The effective VMX deadline cannot exceed the requested budget, and reloads below 2 are refused. The probe reaches its HLT fallback at either a `0x08000000`-TSC-tick horizon or a `0x01000000`-iteration limit. Only after the nonzero VMX deadline actually expires does the handler move guest RIP to the payload and emit the preemption marker. An HLT or timer exit at the exact fallback RIP reports `guest-timeout`; other unexpected probe exits remain guest-exit errors. Unconditional I/O exiting traps the byte `OUT`; the handler validates direction, size, form, port, and value, advances RIP, and never performs a host port write. CPUID and HLT complete the bounded exit sequence. Any later timer expiry, other unexpected exit, or VM-entry/resume failure is terminal and emits a failure marker.

The boot path copies a bounded Limine memory map and allocates all twelve VMX and toy-guest pages only from `USABLE` memory between 1 MiB and 4 GiB. Bootloader-reclaimable memory is deliberately excluded because it can still contain Limine responses and active bootloader page tables.

After runtime and guest pages are materialized through the HHDM, the final Intel path requires NX and four-level paging, sets EFER.NXE and CR0.WP, fills a linker-owned four-page hierarchy, flushes inherited global translations, and switches CR3 before capturing VMCS host state. The live root maps only the linked 2 MiB higher-half kernel window with 4K supervisor leaves: text RX, rodata/GOT R/NX, writable state/stacks/tables RW/NX. Null, HHDM, identity, and five lower stack-guard pages are absent. CR3 and live table contents are read back before `host-paging-ok` is emitted.

The live path remains BSP-only; preflight still uses Limine mappings, and the owned root has no LA57, dynamic/per-CPU mappings, general physical/MMIO policy, teardown, or recovery. It also lacks interrupt injection, APIC and guest-timer virtualization, scheduler-driven preemption, an independent host watchdog, devices, XSAVE state, general guest loading, and hardware qualification. The fixed guest's VMX preemption timer and in-guest fallback are containment checks, not those missing runtime facilities. The signature denylist does not cover unknown timer or TSC errata. A source build or model test is not VMX execution evidence; use matching valid pre/post-run SHA-256 image digests, the strict full marker chain, and the CPU/timer diagnostic audit described in `TYPE1_BOOT_BOUNDARY.md` on a reviewed nested-VMX or bare-metal host.

## Required Exit Coverage

A minimal Linux VMX lab run must cover these exits before it is treated as meaningful lab evidence:

- CPUID;
- HLT;
- port I/O;
- VMX preemption timer;
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
