# Hardware coverage

| Path | Current result |
| --- | --- |
| Replay, translation, device and VMX models | Automated tests; no hardware instructions |
| x86_64 Limine boot under QEMU TCG | Reached descriptor-table setup and runtime preflight |
| Intel VMX fixed guest | Implemented; no retained successful hardware run |
| AMD SVM guest entry | Models and instruction wrappers; not wired into the boot image |
| ARM64 EL2 boot | Models and lab scripts; no bootable image |
| Live tracefs | Manual Linux host check; requires active KVM guests |

Record CPU model, firmware, kernel, virtualization mode, command, image hashes,
serial output, and test result for each host run. One host result does not cover
other firmware or CPUs. Confidential guests are outside current VMI support.

The manual GitHub workflows use self-hosted KVM and AMD runners. Their logs
describe only the checks actually run. The strict Intel run format is in
[kernel notes](TYPE1_BOOT_BOUNDARY.md).
