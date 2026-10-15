# AegisHV

AegisHV is a type-1 hypervisor project and a Linux KVM trace sensor, written in Rust.

The `aegishv` binary runs on the Linux host. It reads `kvm_exit` events from
tracefs, correlates writes followed by execution on the same guest page, and
writes JSONL. It also has Prometheus metrics and policy actions over QEMU QMP.
There is no guest component.

The x86_64 kernel boots through Limine. Its AMD SVM path runs a 64-bit guest
with NPT, register/FPU switching, intercepted syscall MSRs, and read-only code
plus non-executable data. The ARM64 kernel boots at EL2 on QEMU `virt` and runs
an EL1 guest with Stage-2 protection and PMU cycle sampling. Both emit JSONL
events over serial. Intel VMX passes its fixed exit suite in Bochs and nested KVM
on Xeon Platinum 8573C. SVM and EL2 load static ELF64 guests and enforce a timer budget,
including when the guest masks interrupts. These kernels are separate from the Linux sensor.
`tools/type1_events.py` collects their serial JSON and exports Prometheus metrics.

The ARM64 Linux mode boots an unmodified Alpine kernel and root filesystem with
1–4 CPUs, PSCI, virtual interrupts and timers. The guest tests CPU affinity,
shared-memory atomics and CPU hotplug. EL2 records page permission changes,
samples per-CPU PMU cycles and checks exception-vector mappings and contents.
Events use a separate UART outside the guest map. See [Linux boot](docs/LINUX_GUEST.md).
On supported x86 machines, [VT-d](docs/IOMMU.md) starts with all DMA blocked.
The DMA test exercises page permissions and revocation with a QEMU EDU device.

## Build and replay

Rust 1.81 is pinned in `rust-toolchain.toml`. The main workspace has no external
Rust dependencies.

```sh
cargo build --locked --release
./target/release/aegishv run \
  --replay examples/traces/kvm_exit_sample.log --jsonl - --listen ''
```

## Live collection

On a Linux KVM host with tracefs access:

```sh
sudo ./tools/enable_tracefs.sh start /sys/kernel/tracing
sudo ./target/release/aegishv run \
  --tracefs /sys/kernel/tracing --config config.example.toml --jsonl -
```

Stop tracing afterwards with `sudo ./tools/enable_tracefs.sh stop /sys/kernel/tracing`.
The example config uses dry-run and manual approval rules. Set up VM identities
and QMP sockets before enabling actions.

```sh
aegishv validate-config --config config.example.toml
aegishv snapshot --tracefs /sys/kernel/tracing --config config.example.toml
aegishv admin policy-explain --config config.example.toml --json
aegishv dump-schemas --out-dir schema_out
```

The metrics listener serves `/metrics`, `/healthz`, and `/readyz`.
`--listen ''` disables it. A bind failure otherwise stops startup unless
`metrics.allow_bind_failure` is set.

## Limits

W^X alerts are correlations from trace data; the sensor does not change page
permissions. The VMI and detector libraries work with offline snapshots and
fixtures. The userspace sensor's PMU collection currently emits thread heartbeats
with unavailable counters set to `null`; EL2 uses the architectural cycle counter.

The x86 kernels currently run freestanding guests on one CPU. ARM64 Linux uses
an initramfs, 256 MiB RAM and up to four CPUs pinned to host CPUs. Disk, network,
general PCI passthrough and x86 OS loading are unfinished. SVM has passed the
guest suite with nested KVM on AMD EPYC 7763; EL2 has passed under QEMU TCG and
VMX under Bochs and nested KVM. Direct bare-metal runs remain unverified.
See [the kernel notes](docs/TYPE1_BOOT_BOUNDARY.md).

## Tests

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --all --all-features
./scripts/smoke-replay.sh
```

## Source

- `src/`: collector, parser, identity, policies, output, and offline VMI tools.
- `crates/`: kernel, boot handoff, architecture code, and device models.
- `boot/`: assembly entry points, linker script, and Limine config.
- `schema/`: event and snapshot formats.
- `tests/`, `corpus/`, `examples/`: integration tests and replay inputs.

[Architecture](docs/ARCHITECTURE.md) · [Deployment](docs/DEPLOYMENT.md) ·
[Troubleshooting](docs/TROUBLESHOOTING.md) · [Testing](docs/TESTING.md) ·
[Releases](RELEASE.md)

Maintained by [Wythwool](https://github.com/Wythwool) /
[Nullbit1](https://github.com/Nullbit1). MIT license.
