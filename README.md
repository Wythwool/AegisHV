# AegisHV

AegisHV has two deliberately separate runtime surfaces:

- the default `aegishv` binary, a Linux host-side KVM telemetry sensor;
- a separate opt-in, bootable x86_64 Type-1 lab kernel target used for low-level Intel VMX bring-up.

The default sensor reads KVM tracefs `kvm_exit` lines from the host, turns them into structured JSONL events, correlates page-aligned W^X patterns, exports Prometheus text metrics, and can react through QEMU QMP actions. Nothing is installed inside the guest.

## Ownership Metadata

- Creator: https://github.com/Wythwool
- Organization: https://github.com/Nullbit1

These links identify project ownership metadata. They are not a copyright assignment or a legal support statement.

## What this repository is today

- Host-side KVM sensor.
- Separate no-std x86_64 lab kernel with a modern Limine ISO build path, owned GDT/TSS/IDT state, an early physical allocator, and strict serial evidence tooling.
- Wired Intel VMX toy-guest path with VMXON, a complete VMCS, four-level guest paging and EPT, refusal of CPU signatures known to have broken VMX preemption timers, an initial zero-value sentinel followed by a proven nonzero deadline exit from a finite TSC-or-count probe with an HLT fallback, unconditional port-I/O exiting, validated `OUT 0xe9, AL`, CPUID and HLT exits, bounded per-stage resumes, and VMXOFF.
- Replayable parser/correlation pipeline.
- W^X correlation scoped by VM, address space, and guest-physical page.
- Best-effort VM identity enrichment from PID, process start time, QEMU args, cgroups/systemd, UUID/name hints, and QMP socket hints.
- Clean event contract split between trace header `host_cpu` and real guest `vcpu_id`.
- JSONL event output, optional UDP syslog and Linux journald mirroring, an opt-in event spool with plaintext or RLE-compressed segments, and Prometheus-compatible text metrics.
- Policy engine with priority, entity-scoped cooldown, dry-run, suppress, and enforce modes.
- QMP action dispatcher with retries, timeout handling, required stable `vm_id` matching by default, safer dump paths, and mock-tested pause/resume/link actions.
- Committed `Cargo.lock` with a locked, dependency-free main crate.
- CI/release workflows, systemd unit, Docker image, schemas, parser smoke corpus, and docs.

## What this repository is not yet

- The default `aegishv` binary is not a Type-1 hypervisor; it remains the host-side sensor described above.
- The separate lab kernel is not a production or general-purpose Type-1 hypervisor.
- Intel guest execution has not been demonstrated on a reviewed nested-VMX or bare-metal host.
- Not a full VMI stack.
- Not a general direct EPT/NPT/Stage-2 permission enforcement engine; the Intel lab path has only its fixed toy-guest EPT.
- Not a syscall-path integrity product yet.
- Not true hardware PMU sampling; the current no-dependency fallback reports PMU target heartbeat events with unavailable counters as `null`.

That distinction matters. The userspace sensor and bare-metal lab kernel have different entry points, evidence, and release gates. The Type-1 / VMI / trap-engine path is documented in `docs/TYPE1_ROADMAP.md`, with phase backlog items in `BACKLOG.md`.

## Type-1 lab boundary

A modern Limine ISO has booted locally under QEMU TCG through the owned host descriptor tables and runtime preflight. That is boot-boundary evidence only: TCG did not expose VMX in the available environment, and WHPX was unavailable, so the run could not execute the Intel toy guest.

The VMX guest path is present in code, but execution is claimed only after the strict ten-marker chain and validated CPU/timer diagnostic set in `docs/TYPE1_BOOT_BOUNDARY.md` are captured on a reviewed nested-VMX or bare-metal host. Successful evidence would prove one BSP, one fixed guest, a nonzero VMX preemption deadline exit, a trapped and validated port write that is never replayed on the host, one CPUID exit, bounded resumes, and one HLT exit—not production readiness.

Production blockers still include a hypervisor-owned CR3 with enforced W^X and guard pages, SMP/per-CPU VMX, APIC/interrupt/guest-timer virtualization and scheduler-driven preemption, an independent host watchdog, a general guest loader, complete PAT/XSAVE/FPU/MSR context, devices and IOMMU isolation, live AMD/ARM backends, hardware soak, and a secure update/attestation/incident-response lifecycle. The fixed CPU-signature denylist and toy probe cannot rule out unknown timer or TSC errata.

## Highlights in 0.4.0

- Replaced the old bootstrap lock marker with a committed release `Cargo.lock`.
- Removed external dependencies from the main crate so `cargo metadata --locked` is self-contained in this source archive.
- Removed `cargo generate-lockfile` from CI and release paths.
- Added schema-versioned events with monotonic timestamps, sequence numbers, and explicit loss objects.
- Split unsupported/unrelated trace lines from malformed `kvm_exit` parse errors.
- Propagated queue-loss watermarks to the next emitted event through `data_loss=true` and `loss` metadata.
- Tightened QMP action mapping: `identity.require_stable_qmp_match=true` refuses VM-name fallback when no `actions.qmp` `vm_id` pattern matches.
- Added structured action audit metadata for decision, result, attempts, retries, timeout, refusal, and bounded failure class.
- Added PID start-time tracking to reduce PID-reuse identity mistakes.
- Tightened policy cooldown keys: rule + VM scope + reason + trap type + page + action set.
- Fixed optional PMU counters so unavailable counters are `null`, not fake zeroes.
- Fixed `NoHypervisorBackend` so it reports `BackendArch::None` instead of pretending to be Intel VMX.
- Kept tracepoint format autodiscovery as a parser module, while clearly marking text `trace_pipe` as the active ingestion path.

## Quick start

### Build

```bash
cargo metadata --locked --format-version 1
cargo build --locked --release
```

### Replay mode

This is the fastest way to validate parsing, W^X correlation, schemas, and policy logic.

```bash
./target/release/aegishv run \
  --replay ./examples/traces/kvm_exit_sample.log \
  --jsonl - \
  --listen ''
```

### Live mode

Enable KVM tracepoints first:

```bash
./tools/enable_tracefs.sh start /sys/kernel/tracing
```

Then run:

```bash
./target/release/aegishv run \
  --tracefs /sys/kernel/tracing \
  --config ./config.example.toml \
  --jsonl -
```

Stop tracing when you are done:

```bash
./tools/enable_tracefs.sh stop /sys/kernel/tracing
```

## CLI

```bash
aegishv run \
  --tracefs /sys/kernel/tracing \
  --replay <file> \
  --config <config.toml> \
  --jsonl <path-or-dash> \
  --listen <ip:port-or-empty> \
  --queue <size> \
  --quiet \
  --no-quiet
```

```bash
aegishv snapshot --tracefs /sys/kernel/tracing --config ./config.example.toml
aegishv dump-schemas
aegishv dump-schemas --out-dir ./schema_out
aegishv validate-config --config ./config.example.toml
```

## Output contracts

- `schema/event.schema.json` — JSONL events emitted by the runtime pipeline.
- `schema/snapshot.schema.json` — one-shot host snapshot output from `snapshot`, including tracefs diagnostics and bounded VM inventory from configured file-backed identity discovery.

JSONL is the primary event stream. `[syslog] enable = true` can mirror emitted events to one UDP syslog target with bounded facility and severity mapping. `[journald] enable = true` can mirror emitted events to the Linux systemd-journald datagram socket with bounded structured fields. These sinks are not acknowledgement paths, transport retry layers, SIEM schemas, OCSF mappings, or ECS mappings.

Event categories:

- `exit`
- `wx`
- `pmu`
- `policy`
- `sensor`

## W^X correlation

W^X tracking is now:

- page-aligned;
- bounded by `wx_max_pages`;
- detector-level alert cooldown through `wx_cooldown_ms`;
- scoped by VM identity and address space instead of a global GPA-only map;
- compatible with allowlisted noisy/JIT-like patterns.

This is still correlation, not hardware enforcement. It does not flip EPT/NPT/Stage-2 permissions.
W^X detector cooldown is keyed by VM scope, address space, page, and source exit reason. Policy rule cooldown is separate and still runs after a W^X event is emitted.

## Policy engine

Rule features:

- priority ordering;
- strict config validation;
- cooldown scoped by rule, VM, reason, trap type, page, and action set;
- `mode = "enforce"`;
- `mode = "dry_run"`;
- `mode = "suppress"`;
- one or multiple actions per rule.

Supported actions:

- `pause_vm`
- `resume_vm`
- `dump_guest_memory`
- `quarantine_nic`
- `manual_approval`
- `noop`

`dump_guest_memory` is reported as **accepted** when QMP accepts the command. It is not falsely reported as a fully completed dump.

Every policy action emits a `policy` event with reason `policy_action`. The nested `action` object records bounded audit fields: `decision`, `result`, `attempt`, `max_attempts`, `retry_count`, `timeout_ms`, `timed_out`, `refused`, and `failure_class`. `failure_class` is bounded to fixed values such as `qmp_error`, `timeout`, `stable_identity_required`, `unsupported_action`, `unsafe_input`, and `missing_argument`.

## Metrics and health

Endpoints:

- `/metrics`
- `/healthz`
- `/readyz`

The metrics endpoint is a dependency-free Prometheus text exposition endpoint. It includes ingest, parse, unsupported, malformed, queue-loss, W^X, W^X cooldown suppression, policy, QMP, PMU heartbeat, JSON write, syslog and journald write failure, uptime, and health metrics. Queue depth and utilization are approximate gauges maintained by atomic send/receive/drop accounting around the bounded ingest channel.

Trace input reason labels are bounded to `parsed`, `unrelated_tracepoint`, `unsupported_line`, `malformed_kvm_exit`, `parser_degraded`, and `parser_bug`. They must not include raw trace text, VM names, paths, or parser error strings.

Offline VMI infrastructure counters use bounded labels for architecture, translation mode, and typed error kind. They cover synthetic memory reads, translations, register fixture access, profile lookup, fixture loading, and unsupported backend calls. They do not imply live VMI backend coverage and must not include guest addresses, fixture paths, host paths, kernel build strings, VM names, or free-form error text.

`--listen ''` disables the metrics listener. When `--listen` is non-empty, bind failure is fatal by default so the process does not run without the operator-visible health endpoint. Set `metrics.allow_bind_failure = true` only for an explicit degraded startup where missing local metrics is acceptable.

## Testing

The repository includes parser tests for x86/AMD/arm64 replay fixtures, W^X scope tests, config validation tests, metrics encoding tests, QMP mock tests, replay EOF tests including full telemetry queue, corpus regression tests, and parser fuzz-smoke coverage through ordinary locked test runs.

Run locally:

```bash
cargo metadata --locked --format-version 1
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all --all-features
./scripts/smoke-replay.sh
```

## CI / release

Workflow files are present:

- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`

CI verifies the committed lockfile, runs `cargo metadata --locked`, fmt, clippy, tests, nextest, build, smoke replay with event-contract validation, audit/deny, Docker build smoke, and x86_64/aarch64 glibc/musl cross-builds.

Tagging `vX.Y.Z` triggers locked release builds, packaging, SHA-256 generation, and a minimal SBOM artifact.

## Deployment assets

- `systemd/aegishv.service`
- `Dockerfile`
- `config.example.toml`
- `docs/DEPLOYMENT.md`

## Repo layout

- `src/collector.rs` — live/replay ingestion and non-lossy EOF/error control messages.
- `src/parser.rs` — exit parsing and architecture-aware fault decoding.
- `src/identity.rs` — best-effort PID/cgroup/QEMU identity enrichment with PID start-time defense.
- `src/trace_format.rs` — tracepoint format parser/autodiscovery helpers.
- `src/wx.rs` — page-aligned per-VM/address-space W^X engine.
- `src/policy.rs` — policy compilation and matching.
- `src/actions.rs` — QMP actions and mocks.
- `src/metrics.rs` — Prometheus text metrics.
- `src/tracefs.rs` — tracefs helpers and snapshot output.
- `src/vmi.rs` / `src/hypervisor.rs` — live-backend contracts and offline VMI infrastructure; not wired to the bare-metal toy guest.
- `crates/aegishv-hypervisor-core` — `no_std` bare-metal boundary models for IDs, memory maps, page allocation, crash records, per-CPU state, ABI rings, VM lifecycle, and vCPU scheduling.
- `crates/aegishv-event-abi` — `no_std` facade for the event and command ring ABI.
- `crates/aegishv-arch-x86` — `no_std` x86 helpers and models for early serial logging, host page-table plans, AP startup validation, VMX instructions, controls, VMCS state, EPT, and exit handling.
- `crates/aegishv-type1-kernel` — bootable x86_64 lab kernel and the wired Intel VMX toy-guest path.
- `boot/` — Limine configuration, linker layout, early entry, owned host tables, and VMX entry/exit assembly.
- `schema/` — JSON schemas.
- `examples/traces/` — replay fixtures.
- `tests/` — integration tests.
- `BACKLOG.md` — phase-based release and type-1 roadmap with acceptance criteria and gates.
- `docs/` — architecture, threat model, deployment, compatibility, roadmap.
- `AGENTS.md` — maintainer rules for scope, tests, schemas, backend claims, unsafe code, and dependencies.

## Deployment and architecture notes

Read these before making stronger deployment or backend claims:

- `docs/ARCHITECTURE.md`
- `docs/THREAT_MODEL.md`
- `docs/COMPATIBILITY.md`
- `docs/EVENT_EXPORT.md`
- `docs/EVENT_MAPPINGS.md`
- `docs/SECURITY.md`
- `docs/TROUBLESHOOTING.md`
- `docs/TESTING.md`
- `docs/VMI.md`
- `docs/VMI_LINUX.md`
- `docs/TYPE1_ROADMAP.md`
- `docs/TYPE1_BOOT_BOUNDARY.md`
- `docs/TYPE1_READINESS_GATE.md`
- `BACKLOG.md`
- `docs/STATUS.md`
