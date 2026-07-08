# Boot Boundary Artifacts

This directory contains planned type-1 boot artifacts. They define the first image layout, Limine handoff expectations, and x86_64 entry symbol names.

The artifacts are not wired into a bootable image build yet. The normal binary remains the host-side sensor until the runtime entry path, architecture backend, and QEMU evidence are added.

## Files

- `limine/limine.conf` records the first Limine menu entry and kernel path.
- `linker/x86_64-type1.ld` records the planned ELF layout and exported stack symbols.
- `x86_64/entry.S` records the first entry symbol and a halt loop for early bring-up.

`scripts/build-type1-skeleton.sh` validates the boot handoff crate and writes a manifest under `target/type1`. That manifest is review material, not a bootable hypervisor image.

`scripts/plan-type1-image.sh` validates the checked-in boot inputs and writes the current kernel ELF, output image, and QEMU serial-marker contract to `target/type1/aegishv-type1-image-plan.txt`. That manifest is not QEMU boot evidence.

`scripts/build-type1-kernel.sh` builds the minimal `x86_64-unknown-none` kernel ELF to `target/type1/aegishv-type1.elf`. The ELF writes the early serial marker when entered through the checked-in x86_64 stub, but it is not packaged as a bootable ISO yet.
