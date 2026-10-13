# AMD SVM lab

The x86_64 Limine image selects SVM on AMD. It enters a 64-bit guest through
VMRUN, switches GPR/FPU and VMCB state, enforces NPT permissions and stops guests
that exceed the timer budget. Static ELF64 guests use `AEGISHV_GUEST_ELF`.

```sh
cargo test --locked -p aegishv-arch-x86 --all-features svm::
bash scripts/build-type1-limine-iso.sh --build-kernel
python3 scripts/test-type1-boot.py svm target/type1/aegishv-type1.iso
# On an AMD KVM host exposing nested SVM:
python3 scripts/test-type1-boot.py svm target/type1/aegishv-type1.iso --accel kvm
```

The `boot` workflow runs seven guest cases with TCG; `nested-kvm.yml` runs the
same cases through KVM on an AMD host. Both check actual guest exits and retain
serial logs. See [kernel notes](TYPE1_BOOT_BOUNDARY.md) for the memory layout,
ELF examples, QEMU version limits and recorded EPYC runs.

The older `scripts/svm-amd-lab-smoke.sh` checks host SVM and `/dev/kvm`. Its Linux
boot mode still needs a separately supplied `AEGISHV_SVM_LAB_KERNEL`.
`amd-hardware.yml` runs that host check, not the AegisHV guest suite.

SEV, SEV-ES, and SEV-SNP guest inspection is unsupported. Models and host checks
do not establish access to protected guest state.
