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
events over serial. Intel VMX has a separate entry path that still needs a
hardware run. SVM and EL2 load static ELF64 guests and enforce a timer budget,
including when the guest masks interrupts. These kernels are separate from the Linux sensor.
`tools/type1_events.py` collects their serial JSON and exports Prometheus metrics.

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
fixtures. PMU collection currently emits thread heartbeats with unavailable
counters set to `null`.

The kernels currently run small freestanding guests on one CPU. General OS loading, SMP,
device backends and IOMMU isolation are unfinished. SVM has passed the guest suite
with nested KVM on AMD EPYC 7763; EL2 has passed under QEMU TCG. Direct bare-metal
runs and Intel VMX guest execution remain unverified.
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
