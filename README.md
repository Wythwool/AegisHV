# AegisHV (dev build)

Minimal multiboot2 kernel that boots under GRUB and logs to COM1/0xE9.
VMX wiring is intentionally deferred; the run loop halts. This build is meant to compile and boot cleanly.

## Build (host needs: clang, lld, grub-mkrescue, xorriso)
```bash
cd hv
make
cd ..
scripts/build_iso.sh   # produces AegisHV.iso
```

## Run (QEMU)
```bash
qemu-system-x86_64 -serial stdio -cdrom AegisHV.iso -no-reboot -no-shutdown
```
You should see lines like:
```
[INFO] AegisHV boot
[WARN] VMX not supported by CPU; skipping VMXON
[INFO] Init complete; entering loop
```
