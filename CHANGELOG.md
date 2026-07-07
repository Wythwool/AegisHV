# Changelog

## Unreleased

- Added local management CLI commands for version, health, policy review, policy dry-runs, and action dry-runs.
- Added role, audit, approval, policy bundle, dump evidence, and startup hash helper primitives.
- Added benchmark helper scripts for replay ingest, W^X state handling, offline VMI translation, and synthetic trap transitions.
- Added hardware, performance, security, release, VMI alpha, and type-1 readiness gate documents.
- Added planned type-1 boot boundary artifacts: boot handoff crate, Limine config, x86_64 linker script, x86_64 entry symbol, and build-plan helper.
- Expanded synthetic Linux and Windows VMI fixture corpus.

## 0.4.0

- Replaced the bootstrap lock marker with a committed production `Cargo.lock`.
- Removed dependency generation from CI/release paths; locked builds now use the committed lockfile.
- Made the main crate dependency-free so the source archive is self-contained for `cargo metadata --locked`.
- Added event `sequence`, `monotonic_ms`, and structured `loss` metadata.
- Split unsupported/unrelated trace lines from malformed `kvm_exit` parse errors.
- Propagated queue-loss watermarks onto the next emitted event.
- Added PID start-time based identity cache defense.
- Changed QMP action dispatch to prefer stable `vm_id` mappings over VM-name fallback.
- Scoped policy cooldowns by rule, VM, reason, trap type, page, and action set.
- Made unavailable PMU counters serialize as `null` instead of fake zeroes.
- Fixed `NoHypervisorBackend` to report `BackendArch::None`.
- Updated CI, Docker, release, schema, and docs for locked reproducible builds.

## 0.3.1

- Hardened collector EOF/error control messages.
- Split host `host_cpu` from guest `vcpu_id`.
- Added best-effort VM identity enrichment.
- Added W^X page alignment and per-VM/address-space scoping.
- Added strict config validation, policy modes, QMP action tests, and deployment docs.
