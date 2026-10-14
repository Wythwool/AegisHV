# Hardware coverage

| Path | Current result |
| --- | --- |
| Replay, translation, device and VMX models | Automated tests; no hardware instructions |
| x86_64 Limine boot under QEMU TCG | SVM guest suite passes; VMX stops at preflight |
| Intel VMX fixed guest | Full exit suite passes in Bochs and nested KVM on Xeon Platinum 8573C |
| AMD SVM guest entry | Seven cases pass under TCG and nested KVM on EPYC 7763 |
| ARM64 EL2 boot | Seven cases pass under QEMU TCG, Cortex-A57 and GICv2 |
| ARM64 Linux/SMP | Unmodified Alpine kernel; 1–4 CPUs, atomics, hotplug and UART IRQ affinity pass under TCG |
| VT-d DMA | Quarantine, page round trip, permissions and revocation pass with QEMU VT-d/EDU |
| Direct bare-metal boot | No retained physical-host run |
| Live tracefs | Manual Linux host check; requires active KVM guests |

Record CPU model, firmware, kernel, virtualization mode, command, image hashes,
serial output, and test result for each host run. One host result does not cover
other firmware or CPUs. Confidential guests are outside current VMI support.

`boot.yml` runs SVM/EL2 guests on Linux with TCG. The manual `nested-kvm.yml`
uses a runner exposing `/dev/kvm`; SVM has passed on Ubuntu 24.04 and 26.04.
`os-boot.yml` runs ARM64 Linux, VMX in Bochs and VT-d on Linux runners.
The older sensor/host-check workflows use self-hosted KVM and AMD runners.
[Kernel notes](TYPE1_BOOT_BOUNDARY.md) link retained runs and describe the strict
Intel run format.
