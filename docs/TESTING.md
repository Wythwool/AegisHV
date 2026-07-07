# Testing

Local locked test path:

```bash
cargo metadata --locked --format-version 1
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all --all-features
./scripts/smoke-replay.sh
```

Detector engine tests are normal locked Rust tests:

- `detectors_core_tests` covers scoring, scheduler enable flags, unsupported/degraded outcomes, and budget accounting.
- `detectors_normalizer_tests` covers kernel text, syscall hook, and W^X normalization.
- `detectors_inventory_memory_tests` covers hidden process/module comparison, executable anonymous memory, RWX mappings, JIT allow rules, and malformed ranges.
- `detectors_state_incident_tests` covers dedupe, incident correlation, versioned state round trips, and corrupt-state sensor events.

Trap engine tests are normal locked Rust tests:

- `trap_stage2_model_tests` covers permission bits, backend limits, synthetic table lookup, overlap rejection, permission updates, and one-level splits.
- `trap_controller_tests` covers execute/write lifecycles, JIT temporary-window refusal, storm throttling, invalidation planning, and single-step capability selection.
- `trap_benchmark_tests` runs the synthetic benchmark harness with a small iteration count to keep the binary path compiling and executable.

Synthetic trap benchmark harness:

```bash
cargo run --locked --bin trap_synthetic_bench -- --iterations 10000
```

The harness reports local process timing only. It does not benchmark VM exits, hardware invalidation, EPT/NPT writes, or guest runtime behavior.

## Type-1 Boundary Model Crates

The type-1 boundary workspace crates are library models. They are included in the normal workspace gate:

```bash
cargo test --locked -p aegishv-hypervisor-core -p aegishv-event-abi -p aegishv-arch-x86
```

They cover memory-map validation, physical page allocation, crash records, event and command rings, VM lifecycle, vCPU scheduling, early serial logging, x86 page-table plans, and AP startup plan validation. These tests do not boot a hypervisor.

`scripts/type1-qemu-smoke.sh` is opt-in lab plumbing for a boot image once one exists. The repository does not currently ship `./target/type1/aegishv-type1.elf`.

```bash
scripts/type1-qemu-smoke.sh --print-command ./target/type1/aegishv-type1.elf
```

The script exits with a clear error when the image is missing. It is not wired into normal CI.

## Deterministic Replay

Use `--deterministic-replay` only with `--replay` when generating golden JSONL fixtures. It freezes event timestamps, monotonic time, event sequence, event IDs, action IDs, and host/sensor/tenant IDs. Live tracefs runs reject this flag; AegisHV does not fake deterministic timing for live runtime output.

Example:

```bash
cargo run --locked -- run --replay ./examples/traces/kvm_exit_sample.log --deterministic-replay --jsonl ./golden.jsonl --listen '' --quiet
```

Committed golden fixtures live under `tests/fixtures/golden`. Replay-backed fixtures must be regenerated with the commands in that directory's README and compared by tests. Contract-only fixtures must explain why deterministic replay cannot produce that event class.

## Live Tracefs Smoke

Live tracefs smoke is opt-in and host dependent. It requires Linux tracefs, a KVM-capable host, permissions to read and write the KVM tracepoint controls, and enough guest activity to produce a `kvm_exit` event.

```bash
sudo ./scripts/live-tracefs-smoke.sh --timeout 30
```

The script enables `events/kvm/kvm_exit`, writes a trace marker, and waits for a `kvm_exit` line from `trace_pipe` after that marker. It restores the previous `kvm_exit/enable` and `tracing_on` values before exiting. It fails if tracefs is missing, permissions are insufficient, the tracepoint metadata is not readable, or no live KVM exit is observed within the timeout. This smoke is separate from replay and golden fixture tests, and it does not prove type-1 support, VMI, EPT/NPT enforcement, syscall-path integrity, or hardware PMU sampling.

CI additionally runs nextest, Docker build smoke, cargo-audit, cargo-deny, and x86_64/aarch64 glibc/musl cross-builds.

## Opt-In Hardware Workflow

`.github/workflows/hardware.yml` is a manual workflow for reviewed live-host checks. It uses `workflow_dispatch` only and is not triggered by normal `pull_request` or `push` events. The default runner label is `aegishv-hardware-kvm`; operators must point it at a Linux self-hosted runner with the required access.

Prerequisites for meaningful live checks:

- Linux runner with the requested self-hosted label;
- KVM-capable host with `/dev/kvm` present when KVM behavior is being checked;
- mounted tracefs at `/sys/kernel/tracing` or `/sys/kernel/debug/tracing`;
- permission for the runner user to read tracefs metadata and, when `run_live_tracefs` is enabled, write the KVM tracepoint controls used by `scripts/live-tracefs-smoke.sh`;
- enough guest activity to produce a real `kvm_exit` line during the smoke window;
- optional libvirt/QMP permissions only when an operator extends the manual workflow for those local checks.

The manual workflow always runs locked metadata/build, config validation, deterministic replay-style smoke, and event schema validation on the selected runner. The snapshot step is controlled by `run_snapshot`. The live tracefs smoke is controlled by `run_live_tracefs` and remains off by default.

This workflow is scaffolding for opt-in live-host evidence. It does not prove type-1 support, full VMI, EPT/NPT enforcement, syscall-path integrity, live libvirt integration, hardware PMU sampling, package install safety, or that every supported distro and CPU path has been tested. A failed manual run may indicate host permissions, tracefs layout, missing guest activity, or runner setup issues rather than a runtime regression.

## Dependency Policy Checks

Normal PR tests do not require private registries, secrets, private services, live KVM, live tracefs, or package install tests for dependency policy. The CI dependency policy job installs cargo-deny with Cargo and runs:

```bash
cargo deny check
```

The policy in `deny.toml` checks the locked dependency graph for RustSec advisories, yanked crates, duplicate crate versions, wildcard dependency requirements, license allowlists, unknown registries, and unknown git sources. The current lockfile contains only the AegisHV crate, so the policy is intentionally strict: future third-party dependencies must update `Cargo.lock`, justify the dependency, and update the policy or docs when a new license/source exception is required.

The allowed license list is explicit and does not approve every OSI or FSF license by category. AegisHV's MIT package license does not approve future dependency licenses for commercial or redistribution use. cargo-deny is a static dependency policy check. It does not prove vulnerability absence, license approval, supply-chain security, reproducible builds, runtime safety, type-1 support, full VMI, EPT/NPT enforcement, syscall-path integrity, live libvirt integration, or hardware PMU support.

## Release SBOM Checks

Normal PR tests do not install an SBOM tool. The release workflow installs Syft `v1.18.1` and runs:

```bash
bash ./scripts/package-release.sh x86_64-unknown-linux-gnu
bash ./scripts/generate-sbom.sh x86_64-unknown-linux-gnu
```

`scripts/generate-sbom.sh` scans the per-target package directory under `dist/` and writes `dist/aegishv-${version}-${target}.sbom.cdx.json`. The package includes `Cargo.toml` and `Cargo.lock` so the locked Rust dependency graph is available to Syft. If Syft is unavailable, the script exits instead of writing placeholder JSON. The script also rejects missing package inputs, non-CycloneDX output, and generated SBOM text that contains `target/`, `.git/`, cache directories, or the local checkout path.

The SBOM is generated release metadata. It is not a vulnerability scan, license approval, artifact provenance, signing, or runtime support evidence for unsupported backends.

## Release Provenance Checks

Normal PR tests do not generate provenance. The release workflow uses GitHub artifact attestations:

- `actions/attest@v4.1.0` runs after the per-target checksum file is written;
- `subject-checksums: dist/SHA256SUMS-${target}.txt` identifies the release tarball and CycloneDX SBOM subjects;
- `scripts/collect-provenance-bundle.sh` copies the generated Sigstore bundle to `dist/aegishv-0.4.0-x86_64-unknown-linux-gnu.slsa-provenance.sigstore.json` for the matching target;
- the release upload includes `*.slsa-provenance.sigstore.json`.

The collect script rejects a missing bundle, an empty bundle, invalid JSON, and provenance text containing `target/`, `.git/`, cache directories, or the local checkout path. It does not create provenance. If `actions/attest` fails or does not return a bundle path, the release job fails.

Provenance is separate from checksums and Cosign blob signatures. It records how GitHub Actions says the artifact was built and which digests were attested. It does not prove reproducible builds, source review quality, vulnerability status, license approval, or runtime behavior. Verification instructions live in `RELEASE.md`.

## Debian Packaging Checks

Normal PR tests do not install the Debian package and do not require root. Packaging tests inspect the files under `packaging/debian` and `scripts/package-debian.sh` for:

- control metadata and an honest package description;
- binary, config, schema, systemd unit, tmpfiles, docs, and operator script install entries;
- `aegishv` user/group creation guarded by `getent`;
- `/var/lib/aegishv`, dump, spool, log, and runtime directories created with mode `0750`;
- no automatic `systemctl enable` or service start in maintainer scripts;
- no old archives, local user paths, private keys, or fake backend claims.

To build a Debian package on a host with `dpkg-deb`:

```bash
cargo build --locked --release
bash ./scripts/package-debian.sh x86_64-unknown-linux-gnu
```

The helper supports `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`. It rejects musl targets because this packaging path is for Debian GNU/Linux packages.

## RPM Packaging Checks

Normal PR tests do not install the RPM package and do not require root or `rpmbuild`. Packaging tests inspect the files under `packaging/rpm` and `scripts/package-rpm.sh` for:

- spec metadata and an honest package description;
- binary, config, schema, systemd unit, tmpfiles, docs, and operator script install entries;
- `aegishv` user/group creation guarded by `getent`;
- `/var/lib/aegishv`, dump, spool, log, and runtime directories created with mode `0750`;
- no automatic `systemctl enable` or service start in scriptlets;
- no old archives, local user paths, private keys, or fake backend claims.

To build an RPM package on a host with `rpmbuild` and the target Rust toolchain:

```bash
cargo build --locked --release
bash ./scripts/package-rpm.sh x86_64-unknown-linux-gnu
```

The helper supports `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`. It rejects musl targets because this packaging path is for RPM GNU/Linux packages.

## Container Metadata Checks

Normal PR tests do not publish or sign container images. The Docker build smoke in CI only verifies that the Dockerfile still builds.

Container metadata tests inspect the Dockerfile, `.dockerignore`, release docs, and workflow files for:

- standard OCI label keys for source, revision, version, licenses, title, description, and created timestamp;
- creator and organization labels using concrete AegisHV links;
- `.dockerignore` entries that exclude build outputs, repository metadata, caches, `dist/`, package outputs, and old archives from the build context;
- explicit documentation that container image publishing and signing are not implemented in the current release workflow;
- no fake container publishing, fake signing, private keys, local user paths, or unsupported backend claims.

If container publishing is added later, tests should require real registry push wiring and real Sigstore/Cosign signing of the image digest. Checksums or text files are not container signatures.

## Seccomp Profile Checks

Normal PR tests do not require root, Docker, container runtime seccomp enforcement, live KVM, live tracefs, or live libvirt. Seccomp tests inspect `packaging/seccomp/aegishv-seccomp.json`, docs, Debian packaging, and RPM packaging for:

- a default-deny OCI seccomp profile with expected architectures;
- syscall groups for process startup, config/tracefs reads, JSONL/spool/snapshot writes, polling, metrics listener, QMP Unix sockets, syslog, and journald;
- absence of high-risk kernel or privilege syscalls such as `bpf`, `ptrace`, `perf_event_open`, `mount`, module loading, reboot, and keyring syscalls;
- package installation of the profile as an optional file without enabling it in service defaults;
- docs that require operator review and state what the profile permits, blocks, and can break;
- no fake sandbox, exploit-prevention, type-1, full VMI, EPT/NPT enforcement, syscall integrity, live libvirt, or hardware PMU claims.

The tests do not prove the profile works on every distro or container runtime. Operators still need to run replay, live tracefs smoke, metrics listener checks, configured QMP action dry runs, syslog/journald output checks, spool checks, and snapshot checks under the profile before enforcing it.

## AppArmor Profile Checks

Normal PR tests do not require root, AppArmor enforcement, live KVM, live tracefs, live libvirt, package installs, or distro-specific AppArmor tooling. AppArmor tests inspect `packaging/apparmor/usr.bin.aegishv`, docs, Debian packaging, and RPM packaging for:

- expected path categories for config, schemas, tracefs, procfs identity reads, JSONL, spool, dumps, snapshots, QMP sockets, syslog, journald, and runtime state;
- network rules for metrics listener, UDP syslog, and Unix sockets;
- explicit denial of broad sensitive paths such as `/root`, home-directory writes, `/tmp` execution, and `/etc/shadow`;
- package installation of the profile as an optional file without enabling it in service defaults;
- docs that require operator review and state what the profile permits, denies, can break, and must be adjusted per deployment;
- no fake sandbox, exploit-prevention, type-1, full VMI, EPT/NPT enforcement, syscall integrity, live libvirt, or hardware PMU claims.

The tests do not prove AppArmor enforcement on any host. Operators still need to load the profile in complain mode, run replay, live tracefs smoke, metrics listener checks, configured QMP action dry runs, syslog/journald output checks, spool checks, dump-path checks, and snapshot checks, then review audit denials before enforcing it.

## SELinux Policy Skeleton Checks

Normal PR tests do not require root, SELinux enforcement, live KVM, live tracefs, live libvirt, package installs, `checkpolicy`, `semodule`, or distro-specific SELinux tooling. SELinux tests inspect `packaging/selinux`, docs, Debian packaging, and RPM packaging for:

- a readable `aegishv_t` domain, `aegishv_exec_t` binary type, file contexts, and interfaces;
- expected categories for config, schemas, docs, tracefs/debugfs/procfs reads, JSONL, spool, dumps, snapshots, QMP sockets, syslog, journald, metrics networking, and runtime state;
- explicit read coverage for common SELinux trace labels, including `tracefs_t` and `debugfs_t`;
- package installation of the policy skeleton as optional review material without loading it or enabling enforcement in service defaults;
- docs that explain how operators build, inspect, install, test, and adjust the skeleton per deployment;
- no fake confinement, exploit-prevention, type-1, full VMI, EPT/NPT enforcement, syscall integrity, live libvirt, or hardware PMU claims.

The tests do not prove that the policy compiles or confines AegisHV on any host. Operators still need to build it with their distribution's SELinux policy tooling, run it in permissive mode, exercise replay, live tracefs, metrics, QMP dry runs, syslog/journald output, spool, dump-path checks, and snapshots, then review audit denials before enforcing it. Distro-specific tracefs/debugfs labels may require local policy changes.

## Cargo-Fuzz Harness Checks

Normal PR tests do not run cargo-fuzz campaigns. The `fuzz/` package contains local cargo-fuzz harnesses for parser-adjacent inputs and small seed corpora. Inspection tests verify the harness files, target names, corpus presence, and documentation claims.

Install cargo-fuzz locally:

```bash
cargo install cargo-fuzz
```

Run short local campaigns from the repository root:

```bash
cargo fuzz run trace_parser_line -- -max_total_time=60
cargo fuzz run config_input -- -max_total_time=60
cargo fuzz run trace_format_metadata -- -max_total_time=60
cargo fuzz run qmp_action_safety -- -max_total_time=60
```

Current targets:

- `trace_parser_line` exercises `parser::parse_line`, parsed-exit classification, parser degradation checks, and GPA page normalization on bounded UTF-8 trace lines.
- `config_input` writes bounded UTF-8 TOML input to a temporary file and calls `Config::load`.
- `trace_format_metadata` exercises `trace_format::parse_tracepoint_format` on bounded UTF-8 tracepoint metadata.
- `qmp_action_safety` constructs an `ActionDispatcher` with a fixed QMP mapping and exercises bounded action refusal paths for missing, ambiguous, stale, PID-only, low-confidence, conflicting, and unverified identity states. It does not connect to a QMP socket.

The QMP action-safety corpus uses `.seed` files. The first byte intentionally maps through `selector % 7` to one refusal reason, and the fourth byte is even so the harness uses `execute=true` rather than dry-run.

The Python JSON Schema validator is not fuzzed by this structure. The committed validator tests remain the schema compatibility gate. Adding a Rust JSON Schema dependency stack should be reviewed separately if a future harness needs it.

The normal CI fuzz harness smoke runs `cargo check --manifest-path fuzz/Cargo.toml --bins`. It type-checks the harness package and does not run fuzz campaigns.

These harnesses do not require live KVM, live tracefs, live QEMU, libvirt, root, network services, secrets, package installs, or host-specific paths. They do not prove vulnerability absence, parser completeness, runtime safety, type-1 support, full VMI, EPT/NPT enforcement, syscall-path integrity, live libvirt integration, or hardware PMU support.

## Release Signing Checks

Normal PR tests do not require signing keys or GitHub OIDC. The release workflow uses keyless Sigstore signing:

- `id-token: write` grants the job an OIDC token;
- `sigstore/cosign-installer@v4.1.0` installs Cosign `v3.0.5`;
- `scripts/sign-release-artifacts.sh` runs `cosign sign-blob --yes --bundle`;
- the release tarball, CycloneDX SBOM, and `SHA256SUMS-${target}.txt` each get a `*.sigstore.json` signature bundle.

The signing script fails if Cosign is missing, if an expected artifact is missing or empty, or if Cosign does not create a bundle. Checksums are still generated, but checksums are not signatures. Verification instructions live in `RELEASE.md`.

## Current automated coverage

- Parser fixtures for x86 EPT-like, AMD NPF-like, and arm64 Stage-2-like lines.
- Unsupported/unrelated line classification versus malformed `kvm_exit` errors.
- W^X page alignment, same-VM correlation, cross-VM guard, and cross-address-space guard.
- Config validation and clamping.
- Policy priority, suppress/dry-run behavior, and entity-scoped cooldown.
- QMP mock success/error path and dump-path rejection for parent traversal, missing parents, existing files, paths outside `dump_root`, unsafe dump roots, and symlinks.
- Structured action audit fields for dry-run, success, stable-match refusal, QMP retry failure, and timeout paths.
- Replay EOF control messages, including full telemetry queue.
- Dependency-free JSON Schema validation for replay JSONL and snapshot JSON output, including bounded VM inventory fields.
- Golden JSONL fixtures for deterministic replay output and event contracts.
- Live tracefs smoke script contract checks. The live smoke itself is opt-in because it needs host tracefs access and KVM guest activity.
- Tracefs format diagnostics for healthy, missing, malformed, and missing-field KVM `kvm_exit` metadata.
- Bounded trace input metric reason labels for parsed, unrelated, unsupported, malformed, degraded, and parser-bug buckets.

## Still required before real type-1/EDR claims

- Hardware-in-the-loop Intel VMX, AMD SVM, and ARM64 EL2 tests.
- Real KVM/libvirt lifecycle tests are still required.
- Binary/perf trace ingestion tests.
- Real PMU grouped-counter/ring-buffer sampling tests.
- Guest memory/register/page-walk tests.
- Linux/Windows syscall-path integrity fixtures are still required.
- Trap enforcement correctness and performance tests.
