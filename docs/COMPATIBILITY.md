# Compatibility

| Component | Support |
| --- | --- |
| Host sensor | Linux KVM tracefs text; replay works without a KVM host |
| Trace parsers | Intel EPT-style exits, AMD NPF, ARM64 Stage-2 abort text |
| Identity | `/proc`, QEMU arguments, cgroups, domain XML snapshots with host task metadata |
| Response | QMP actions through configured sockets |
| PMU | `/proc` thread heartbeat; hardware counters unavailable |
| VMI | Offline memory/register/profile inputs and translation |
| x86_64 kernel | Limine boot; fixed Intel VMX guest path needs hardware validation |
| AMD/ARM kernel execution | Not wired |

The Linux release matrix builds x86_64/aarch64 with glibc and musl. Cross-builds
do not test live tracefs or hardware virtualization. Windows can run portable
tests and replay; Linux-only sinks and process discovery need Linux.

Tracepoint text varies by kernel. Retain replay samples for the host kernels in
use. Huge pages, nested virtualization, migration, and VM lifecycle changes
need deployment-specific checks. Libvirt daemon discovery is not implemented.

Offline profiles use exact identities; there is no nearest-match fallback.
Confidential/protected guests can hide memory or registers and are unsupported.
See [hardware coverage](HARDWARE_TEST_MATRIX.md) for the kernel and lab paths.
