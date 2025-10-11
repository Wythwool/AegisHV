# BUILD

## Dev Harness (Linux, x86_64)
- Rust 1.77+
- libclang for bindgen (if needed), libcap
- Linux with /dev/kvm
```
sudo apt install build-essential libssl-dev pkg-config llvm clang
cargo build -p aegisd --release
cargo run -p devharness
```

## Driver
```
make -C drivers/linux
sudo insmod drivers/linux/aegishv.ko
ls -l /dev/aegishv
```

## Microvisor (bare‑metal, experimental)
This is scaffolding only. You need a boot chain (UEFI/EDK2 or a minimal 64‑bit loader), identity maps,
proper VMCS, AP bring‑up, and EPT/NPT S2 tables. Start at `hv/x86/boot/entry64.S` and wire `vmx_init()`.

**You will brick nothing** if you test in a lab and have a recovery path, but do not flash random builds
to prod boxes. Bring‑up requires a serial console and patience.
