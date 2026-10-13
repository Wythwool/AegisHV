# Boot files

- `limine/limine.conf`: ISO menu and kernel path.
- `linker/x86_64-type1.ld`: RX/R/RW segments, stacks, guards, host page-table pool,
  and exported layout symbols.
- `x86_64/entry.S`: initial IDT, BSS clearing, stack switch, descriptor setup,
  and Rust entry.
- `x86_64/host_tables.S`: GDT/TSS/IDT, selector reload, and terminal fault path.
- `x86_64/vmx_entry.S`: VMLAUNCH/VMRESUME and the VM-exit register frame.

Build with `scripts/build-type1-kernel.sh`. The ELF requires a Limine handoff;
passing it directly to QEMU `-kernel` is insufficient. Use the Limine ISO builder
and runner described in [kernel notes](../docs/TYPE1_BOOT_BOUNDARY.md).
