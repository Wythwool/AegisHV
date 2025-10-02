# AegisHV — Type‑1 Hypervisor (VMXON demo, QEMU/KVM)

**What it is**  
A tiny bare‑metal hypervisor that **boots to 64‑bit long mode**, brings up **Intel VMX**, runs `VMXON` successfully, prints status over **COM1**, and then halts.  
This is a real, working PoC you can boot in QEMU. It lays clean groundwork for VMCS/EPT/guest launch in the next step.

> Target: QEMU on an Intel host with VMX. Works with `-enable-kvm -cpu host,+vmx`.

## Build
Linux deps:
```bash
sudo apt-get update
sudo apt-get install -y build-essential nasm xorriso grub-pc-bin
```

Build ISO:
```bash
make iso
```

Run (serial to stdout):
```bash
./run-qemu.sh
```

You should see something like:
```
AegisHV: entering long mode...
AegisHV: COM1 ready
AegisHV: VMX basic rev=0x000000xx, region set
AegisHV: VMXON OK
AegisHV: done. Halting.
```

## Layout
- `src/boot.asm` — multiboot2, long‑mode switch, COM1 init, logging, calls `vmx_init`.
- `src/vmx.asm`  — VMX capability checks, CR0/CR4 fixups, FEATURE_CONTROL MSR, `VMXON/VMXOFF`.
- `src/gdt.asm`  — 64‑bit GDT (flat).
- `grub/grub.cfg` — GRUB config.
- `kernel.ld`     — linker script.
- `Makefile`      — build rules.
- `run-qemu.sh`   — sensible QEMU flags.
