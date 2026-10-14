# Testing

```sh
cargo metadata --locked --format-version 1
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --all --all-features
./scripts/smoke-replay.sh
./scripts/check-doc-links.sh
```

The workspace tests cover parsing, W^X scope/cooldown, configuration, identity,
QMP mocks and refusal paths, queue loss, shutdown, schema output, offline VMI,
and the kernel's allocation and VMX models. Unix-only paths require Linux to run.

## Replay

```sh
cargo run --locked -- run --replay examples/traces/kvm_exit_sample.log \
  --deterministic-replay --jsonl target/replay.jsonl --listen '' --quiet
python3 scripts/validate-jsonl-schema.py \
  --schema schema/event.schema.json --jsonl target/replay.jsonl
```

Golden files are in `tests/fixtures/golden`; benign and suspicious W^X inputs are
in `corpus`. Deterministic replay fixes variable event fields for comparisons.

## Kernel

```sh
bash scripts/check-type1-lab-tools.sh
bash scripts/build-type1-kernel.sh
bash scripts/stage-type1-limine-iso.sh
```

The build requires `x86_64-unknown-none` and `llvm-tools-preview`.
`scripts/inspect-type1-kernel.sh` checks the ELF and rejects host FPU/SIMD
instructions. Staging copies the kernel and config; it does not create a
bootable ISO. `scripts/build-type1-limine-iso.sh` additionally needs Limine and
xorriso. [Kernel notes](TYPE1_BOOT_BOUNDARY.md) cover the full boot command.

Unit tests and compilation do not execute VMX, SVM, or EL2 instructions.
`scripts/type1-qemu-smoke.sh` checks serial markers;
`scripts/type1-qemu-evidence.sh` also records and validates the image digest and
timer diagnostics for Intel. `scripts/test-type1-vmx.py` runs the same guest in
Bochs and validates the complete exit sequence and timer diagnostics.

`boot.yml` builds and executes seven cases each for SVM and EL2 on Linux QEMU TCG.
The cases check computed results, preemption with IRQs masked, code write denial,
data execute denial, isolation and invalid syscall targets. The runner checks
exit codes, serial events and image hashes. It also tests the serial collector
and converts the actual boot logs to Prometheus output.

`nested-kvm.yml` is a manual run with hardware acceleration. It selects the strict
Intel test on a VMX host, or the seven SVM cases on an AMD host. The available
AMD hosts have passed; physical Intel and Arm remain unverified.

`os-boot.yml` runs an unmodified Alpine ARM64 kernel with 1, 2 and 4 CPUs,
CPU affinity, shared-memory atomics and CPU hotplug. It also runs the VMX suite
in Bochs and checks DMA denial, a page round trip, read-only permissions and
revocation with QEMU VT-d/EDU. Each runner validates serial results and retains
the tested image hashes. See [Linux](LINUX_GUEST.md) and [DMA](IOMMU.md).

## Host checks

`scripts/live-kvm-integration.sh` requires `AEGISHV_RUN_LIVE_KVM=1`, Linux KVM,
tracefs permissions, and enough guest activity to produce exits.
`scripts/live-tracefs-smoke.sh --timeout 30` checks live trace collection.

The `hardware.yml` and `amd-hardware.yml` workflows are manual and use
self-hosted runners. They do not run for pull requests. See
[hardware coverage](HARDWARE_TEST_MATRIX.md) and the
[Intel](VMX_LAB.md), [AMD](SVM_LAB.md), and [ARM](ARM64_LAB.md) notes.

## Fuzzing and benchmarks

The separate [fuzz package](../fuzz/README.md) covers trace parsing, trace format
metadata, config input, and QMP validation. It uses libFuzzer and has its own
dependencies. `cargo check --manifest-path fuzz/Cargo.toml --bins` only compiles
the harnesses.

[Benchmark scripts](PERFORMANCE.md) exercise replay, translation, and synthetic
traps. They do not measure hardware VM-exit cost.
