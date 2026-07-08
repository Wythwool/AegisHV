# Boot Boundary Artifacts

This directory contains planned type-1 boot artifacts. They define the first image layout, Limine handoff expectations, and x86_64 entry symbol names.

The artifacts are not wired into a bootable image build yet. The normal binary remains the host-side sensor until the runtime entry path, architecture backend, and QEMU evidence are added.

## Files

- `limine/limine.conf` records the first Limine menu entry and kernel path.
- `linker/x86_64-type1.ld` records the planned ELF layout and exported stack symbols.
- `x86_64/entry.S` records the first entry symbol and a halt loop for early bring-up.
- The kernel ELF carries a writable `.limine_requests` section with base revision, delimiter, memory-map, HHDM, executable-address, RSDP, bootloader-info, and command-line requests for the first lab handoff.

`scripts/build-type1-skeleton.sh` validates the boot handoff crate and writes a manifest under `target/type1`. That manifest is review material, not a bootable hypervisor image.

`scripts/plan-type1-image.sh` validates the checked-in boot inputs and writes the current kernel ELF, output image, and QEMU serial-marker contract to `target/type1/aegishv-type1-image-plan.txt`. That manifest is not QEMU boot evidence.

`scripts/build-type1-kernel.sh` builds the minimal `x86_64-unknown-none` kernel ELF to `target/type1/aegishv-type1.elf`. The ELF writes the early success marker only after the minimal Limine handoff fields are present; otherwise it writes a separate missing-handoff marker and halts. It is not packaged as a bootable ISO yet.

`scripts/inspect-type1-kernel.sh` checks the built ELF for the expected entry address and `.limine_requests` section when `llvm-readobj` is available, and always checks that both the success marker and missing-handoff marker bytes are present.

`scripts/stage-type1-limine-iso.sh` copies the kernel ELF and `limine.conf` into `target/type1/limine-iso-root`. It records whether `limine` and `xorriso` are available, but it does not create a bootable ISO yet.

`scripts/build-type1-limine-iso.sh` builds `target/type1/aegishv-type1.iso` only when `xorriso`, the `limine` command, and `AEGISHV_LIMINE_DIR` are available. The ISO build still does not count as QEMU boot evidence.
