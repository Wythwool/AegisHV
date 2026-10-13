# AMD SVM lab

`aegishv-arch-x86::svm` contains capability checks, VMCB state, NPT, ASID handling,
instruction wrappers, and mocked runtime tests. The x86_64 boot image does not
wire VMRUN.

```sh
cargo test --locked -p aegishv-arch-x86 --all-features svm::
scripts/svm-amd-lab-smoke.sh --check-host --log-dir target/amd-lab
```

The helper checks AMD SVM and `/dev/kvm`. Its Linux boot mode needs a separately
supplied `AEGISHV_SVM_LAB_KERNEL`. The manual `amd-hardware.yml` workflow keeps
host-check logs; it does not execute an AegisHV SVM guest.

SEV, SEV-ES, and SEV-SNP guest inspection is unsupported. Models and host checks
do not establish access to protected guest state.
