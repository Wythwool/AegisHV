# Type-1 Boot Boundary

This document records the planned type-1 boot boundary now present in the repository. It is a skeleton for later runtime work, not a bootable hypervisor image.

## Present Artifacts

- `crates/aegishv-type1-boot` validates boot handoff data, memory map shape, Limine request expectations, and planned link layout.
- `boot/limine/limine.conf` records the first Limine menu entry and expected kernel path.
- `boot/linker/x86_64-type1.ld` records the planned x86_64 ELF layout and boot stack symbols.
- `boot/x86_64/entry.S` records the first entry symbol and halt-loop fallback for early bring-up.
- `scripts/build-type1-skeleton.sh` validates the boot crate and writes a review manifest under `target/type1`.
- `scripts/plan-type1-image.sh` validates the current image inputs and records the QEMU serial-marker contract.

## Not Present Yet

- Bootable type-1 image is not produced.
- VMXON, VMLAUNCH, VMRESUME, VMRUN, and EL2 entry are not implemented by this milestone.
- AP startup assembly, APIC routing, IDT/GDT runtime setup, and long-mode transition code are not implemented.
- QEMU boot evidence is not present.
- Guest execution, VM exits, EPT/NPT/Stage-2 permission updates, and live VMI are not implemented by this boot boundary.

## Next Gate

The next milestone should turn the x86_64 entry skeleton into a minimal lab image that emits `aegishv:type1:halt` under QEMU and halts in a controlled path. That milestone still needs a real kernel ELF, a captured serial log, and negative tests before any runtime claim is made.
