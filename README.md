# AegisHV

KVM trace sensor and an experimental x86_64 kernel, written in Rust.

The `aegishv` binary runs on the Linux host. It reads `kvm_exit` events from
tracefs, correlates writes followed by execution on the same guest page, and
writes JSONL. It also has Prometheus metrics and policy actions over QEMU QMP.
There is no guest component.

The kernel in `crates/aegishv-type1-kernel` boots through Limine and contains a
single-CPU Intel VMX test guest. It is separate from the sensor. VMX guest
execution still needs a hardware run; the existing boot results stop at
preflight under QEMU TCG.

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

The lab kernel has no general guest loader, SMP, device drivers, live AMD/ARM
entry path, or FPU context switching. See [the kernel notes](docs/TYPE1_BOOT_BOUNDARY.md)
before trying it.

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
