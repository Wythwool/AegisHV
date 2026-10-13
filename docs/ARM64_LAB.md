# ARM64 lab

`aegishv-arch-arm64` has EL2 capability, ESR/trap, Stage-2, VTCR/VTTBR, timer,
TLBI, vector, and GIC models. There is no bootable ARM64 kernel in this tree.

```sh
cargo test --locked -p aegishv-arch-arm64 --all-features
scripts/arm64-el2-lab-smoke.sh --check-host --log-dir target/arm64-lab
```

The QEMU helper expects `AEGISHV_ARM64_BOOT_IMAGE`, targets the `virt` machine
with virtualization and GICv3, and can require `/dev/kvm`. Firmware must enter
the kernel at EL2. The current GIC code does not inject live interrupts, and
the DMA model does not program SMMU hardware.

Fault decoding combines syndrome and translation context; FAR_EL2 alone is not
a guest physical address. pKVM, Arm CCA realms, and encrypted guest inspection
are unsupported.
