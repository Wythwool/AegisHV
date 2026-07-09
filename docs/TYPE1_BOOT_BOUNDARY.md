# Type-1 Boot Boundary

This document records the planned type-1 boot boundary now present in the repository. It is a skeleton for later runtime work, not a bootable hypervisor image.

## Present Artifacts

- `crates/aegishv-type1-boot` validates boot handoff data, memory map shape, Limine request expectations, and planned link layout.
- `boot/limine/limine.conf` records the first Limine menu entry and expected kernel path.
- `boot/linker/x86_64-type1.ld` records the planned x86_64 ELF layout, Limine request section, and boot stack symbols.
- `boot/x86_64/entry.S` records the first entry symbol, masks interrupts, clears direction state, zeroes `.bss`, aligns the boot stack, and keeps a halt-loop fallback for early bring-up.
- `scripts/build-type1-skeleton.sh` validates the boot crate and writes a review manifest under `target/type1`.
- `scripts/plan-type1-image.sh` validates the current image inputs and records the QEMU serial-marker and kernel-base contract.
- `crates/aegishv-type1-kernel` builds a minimal `x86_64-unknown-none` kernel ELF that carries the first Limine request block, writes the planned success marker only after the minimal Limine handoff has accepted base revision, response revisions, HHDM offset, nonempty memory-map with entries pointer, and executable-address bases matching the linker layout, reads a bounded CPUID/MSR snapshot for VMX/SVM capability selection, reads CR0/CR4/EFER and VMX fixed-bit MSRs for register preflight planning, applies the controlled CR0/CR4 or EFER write plan, materializes the selected VMXON/VMCS or VMCB runtime pages through HHDM, writes runtime backend, preflight, enable, and region markers, writes specific fallback markers for incomplete handoffs, and halts when its entry path is reached.
- `scripts/build-type1-kernel.sh` writes `target/type1/aegishv-type1.elf` and a kernel build manifest.
- `scripts/inspect-type1-kernel.sh` records local ELF inspection for the expected entry address, section layout, boot stack size, success marker bytes, runtime backend marker bytes, CPUID/MSR probe marker coverage, runtime preflight marker bytes, runtime enable marker bytes, runtime region marker bytes, missing-handoff marker bytes, and status-specific handoff marker bytes.
- `scripts/stage-type1-limine-iso.sh` stages the kernel ELF and Limine config into an ISO-root directory without claiming boot evidence.
- `scripts/build-type1-limine-iso.sh` can build a Limine ISO when external Limine and xorriso tooling is supplied.
- `scripts/check-type1-lab-tools.sh` records local availability for the reviewed ISO and QEMU lab path.
- `scripts/type1-qemu-evidence.sh` wraps the opt-in QEMU smoke path and records the local serial-marker evidence result.
- `scripts/run-type1-lab.sh` chains the local tool gate, Limine ISO build, and QEMU evidence capture behind an explicit lab-run environment flag.

## Not Present Yet

- Bootable type-1 ISO is not produced by default CI because Limine and xorriso are external reviewed tools.
- The type-1 kernel can choose the checked VMX/SVM runtime plan from a CPU capability snapshot, apply the CR0/CR4/EFER values needed before entry, and materialize the selected runtime pages, but it does not call the VMXON/VMLAUNCH/VMRESUME or VMRUN execution paths yet; EL2 entry is not implemented by this milestone.
- AP startup assembly, APIC routing, IDT/GDT runtime setup, and long-mode transition code are not implemented.
- QEMU boot evidence is not present.
- Guest execution, VM exits, EPT/NPT/Stage-2 permission updates, and live VMI are not implemented by this boot boundary.

## Next Gate

The next milestone should run the tool-gated Limine ISO builder on a host with reviewed Limine/xorriso inputs, then emit `aegishv:type1:halt` under QEMU and halt in a controlled path. That milestone still needs a checked serial log, a true QEMU evidence manifest, and negative tests before any runtime claim is made.
