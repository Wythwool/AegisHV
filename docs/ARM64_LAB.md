# ARM64 lab

`aegishv-el2` boots on QEMU `virt` at EL2, installs host and Stage-2 page tables,
and enters an EL1 guest. It handles HVC, memory faults and timer IRQs, saves
GPR/FP/SIMD state, checks VBAR and samples the PMU cycle counter.

```sh
cargo test --locked -p aegishv-arch-arm64 --all-features
bash scripts/build-type1-arm64.sh
python3 scripts/test-type1-boot.py el2 target/type1/aegishv-el2.elf
```

The runner uses Cortex-A57, GICv2 and TCG. The `boot` workflow runs the built-in
probes and six ELF guests, checking actual exits and serial events. Physical
Arm hardware and nested KVM remain untested. Guest interrupt injection, GICv3
and SMMU programming are not wired into this kernel.

The older `scripts/arm64-el2-lab-smoke.sh` is a separate host-check helper using
`AEGISHV_ARM64_BOOT_IMAGE` and GICv3; it does not run this image.
[Kernel notes](TYPE1_BOOT_BOUNDARY.md) describe the ELF layout and boot sequence.

Fault decoding combines syndrome and translation context; FAR_EL2 alone is not
a guest physical address. pKVM, Arm CCA realms, and encrypted guest inspection
are unsupported.
