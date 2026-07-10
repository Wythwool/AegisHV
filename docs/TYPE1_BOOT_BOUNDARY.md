# Type-1 Boot Boundary

This document records the planned type-1 boot boundary now present in the repository. It is a skeleton for later runtime work, not a bootable hypervisor image.

## Present Artifacts

- `crates/aegishv-type1-boot` validates boot handoff data, memory map shape, Limine request expectations, and planned link layout.
- `boot/limine/limine.conf` records the first Limine menu entry and expected kernel path.
- `boot/linker/x86_64-type1.ld` records the planned x86_64 ELF layout, Limine request section, and boot stack symbols.
- `boot/x86_64/entry.S` records the first entry symbol, masks interrupts, clears direction state, zeroes `.bss`, aligns the boot stack, and keeps a halt-loop fallback for early bring-up.
- `scripts/build-type1-skeleton.sh` validates the boot crate and writes a review manifest under `target/type1`.
- `scripts/plan-type1-image.sh` validates the current image inputs and records the QEMU serial-marker and kernel-base contract.
- `crates/aegishv-type1-kernel` builds a minimal `x86_64-unknown-none` kernel ELF that carries the first Limine request block, writes the planned success marker only after the minimal Limine handoff has accepted base revision, response revisions, HHDM offset, nonempty memory-map with entries pointer, and executable-address bases matching the linker layout, copies the bounded Limine memory map, projects only `USABLE` entries into the early allocator, keeps bootloader-reclaimable memory reserved, selects backend-specific pages between 1 MiB and 4 GiB, reads a bounded CPUID/MSR snapshot for VMX/SVM capability selection, reads CR0/CR4/EFER and VMX fixed-bit MSRs for register preflight planning, applies the controlled CR0/CR4 or EFER write plan, materializes the selected VMXON/VMCS or VMCB runtime pages through HHDM, runs a VMXON/VMCLEAR/VMPTRLD/VMXOFF smoke cycle for the Intel VMX backend, writes runtime backend, preflight, enable, region, VMXON-cycle, and VMCS-load markers, writes specific fallback markers for incomplete handoffs, and halts when its entry path is reached.
- `scripts/build-type1-kernel.sh` writes `target/type1/aegishv-type1.elf` and a kernel build manifest.
- `scripts/inspect-type1-kernel.sh` records local ELF inspection for the expected entry address, section layout, boot stack size, success marker bytes, runtime backend marker bytes, CPUID/MSR probe marker coverage, runtime preflight marker bytes, runtime enable marker bytes, runtime region marker bytes, VMXON-cycle marker bytes, VMCS-load marker bytes, missing-handoff marker bytes, and status-specific handoff marker bytes.
- `scripts/stage-type1-limine-iso.sh` stages the kernel ELF and Limine config into an ISO-root directory without claiming boot evidence.
- `scripts/build-type1-limine-iso.sh` can build a Limine ISO when external Limine and xorriso tooling is supplied.
- `scripts/check-type1-lab-tools.sh` records local availability for the reviewed ISO and bounded QEMU lab path, including a compatible timeout command.
- `scripts/type1-qemu-evidence.sh` wraps the opt-in QEMU smoke path and records whether the VMX backend, VMXON cycle, and VMCS load markers appeared in order. It rejects contradictory backend, failure, skipped, missing-handoff, and panic markers and records the effective QEMU execution contract.
- `scripts/run-type1-lab.sh` chains the local tool gate, Limine ISO build, and strict ordered-marker QEMU evidence capture behind an explicit lab-run environment flag.

## Not Present Yet

- Bootable type-1 ISO is not produced by default CI because Limine and xorriso are external reviewed tools.
- The type-1 kernel can choose the checked VMX/SVM runtime plan from a CPU capability snapshot, apply the CR0/CR4/EFER values needed before entry, materialize the selected runtime pages, and run VMXON, VMCLEAR, VMPTRLD, and VMXOFF for the Intel VMX backend, but it does not write VMCS fields, launch or resume a guest, or execute VMRUN; EL2 entry is not implemented by this milestone.
- AP startup assembly, APIC routing, IDT/GDT runtime setup, and long-mode transition code are not implemented.
- QEMU boot evidence is not present.
- Guest execution, VM exits, EPT/NPT/Stage-2 permission updates, and live VMI are not implemented by this boot boundary.

## Next Gate

The next milestone should run the tool-gated Limine ISO builder on a reviewed Intel nested-VMX host, then capture `backend-vmx`, `vmxon-cycle-ok`, and `vmcs-load-ok` in order under QEMU before halting in a controlled path. A `halt` marker alone is not QEMU VMX evidence. That milestone still needs a checked serial log and a true QEMU evidence manifest before any runtime claim is made.
