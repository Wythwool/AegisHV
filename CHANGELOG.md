# Changelog

## Unreleased

- Added local management CLI commands for version, health, policy review, policy dry-runs, and action dry-runs.
- Added role, audit, approval, policy bundle, dump evidence, and startup hash helper primitives.
- Added benchmark helper scripts for replay ingest, W^X state handling, offline VMI translation, and synthetic trap transitions.
- Added hardware, performance, security, release, VMI alpha, and type-1 readiness gate documents.
- Added planned type-1 boot boundary artifacts: boot handoff crate, Limine config, x86_64 linker script, x86_64 entry symbol, and build-plan helper.
- Added type-1 image input planning and a QEMU serial-marker evidence contract for future boot smoke runs.
- Added a minimal x86_64 type-1 kernel ELF build path that writes the planned serial marker and records that ISO/QEMU evidence is still absent.
- Added type-1 kernel ELF inspection and Limine ISO-root staging helpers without claiming bootable ISO or QEMU evidence.
- Added a tool-gated Limine ISO builder and ISO-aware QEMU smoke command construction.
- Added a type-1 lab tool probe for reviewed ISO and QEMU prerequisites.
- Added a QEMU smoke evidence wrapper that records boot image digest, serial marker state, and smoke exit status.
- Added an opt-in type-1 lab runner that chains tool checks, Limine ISO build, and QEMU evidence capture.
- Added the first kernel-side Limine request block and ELF inspection for the `.limine_requests` section.
- Made the early type-1 kernel success marker depend on the minimal Limine handoff being present.
- Tightened the minimal Limine handoff gate to check HHDM offset, nonempty memory-map, and executable-address response fields before the ready marker.
- Added status-specific serial markers for incomplete Limine handoff checks.
- Checked Limine response revisions and memory-map entries pointer before the ready marker.
- Made the type-1 kernel build use static relocation and the x86_64 kernel code model explicitly.
- Tightened the x86_64 type-1 entry stub by clearing direction state and zeroing `.bss` before the Rust entry.
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
