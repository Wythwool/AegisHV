# AGENTS.md

This file applies to the full AegisHV repository.

Creator: https://github.com/Wythwool  
Organization: https://github.com/Nullbit1

## Repository Position

AegisHV 0.4.0 is a Linux userspace KVM telemetry sensor. The active runtime reads KVM tracefs text, emits JSONL events, exposes Prometheus-style metrics, correlates W^X patterns, and can call QEMU QMP actions.

AegisHV is not a type-1 hypervisor, not a full VMI stack, not an EPT/NPT/Stage-2 enforcement engine, not a syscall-path integrity product, and not true hardware PMU sampling. Keep that boundary intact unless a PR implements and tests the exact backend capability being claimed.

## Maintainer PR Rules

- Keep a small scope. One PR should do one reviewable thing.
- Tests required. Add or update focused tests for changed behavior, including negative tests for refusal paths and unsupported behavior when those paths are touched.
- Preserve CLI behavior unless the PR explicitly changes it and documents the operator impact.
- Do not rewrite unrelated modules, reformat untouched files, or move code to make a small change look larger.
- Do not return success for unsupported behavior. Unsupported behavior must return typed unsupported errors that tell an operator or developer what to change.

## Backend Honesty

- no-fake-type1: Do not claim type-1 support unless real type-1 backend code, enforcement behavior, and tests are present in the PR.
- Do not claim full VMI, EPT/NPT/Stage-2 enforcement, syscall-path integrity, hardware PMU sampling, libvirt support, or readiness for production use unless the PR implements and tests that exact behavior.
- Do not add fake backends, fake hardware support, empty modules, success-returning placeholders, or documentation that implies unavailable backend support.
- `src/hypervisor.rs` and `src/vmi.rs` are backend contracts and no-backend boundaries. Keep unsupported paths explicit.

## Schema Discipline

- Do not change `schema/event.schema.json` or `schema/snapshot.schema.json` unless the PR is a schema migration.
- Schema migration discipline means updating schemas, parser or emitter tests, examples, docs, compatibility notes, and replay validation in the same PR.
- Public event fields need stable semantics. Do not rename or repurpose fields without a versioned migration.

## Unsafe Code

- Keep unsafe code out unless there is a concrete kernel, hypervisor, FFI, or memory-layout reason.
- Every unsafe block needs a comment that states the invariant being upheld. Useful comments explain pointer validity, ownership, alignment, aliasing, lifetime, syscall, kernel ABI, or hypervisor assumptions.
- Do not use comments to restate obvious Rust syntax.

## Dependency Policy

- Keep the main crate dependency-free unless a dependency removes real maintenance risk that is larger than the dependency cost.
- If a dependency is added, justify it in the PR, update `Cargo.lock`, keep `cargo metadata --locked` working, and document any operator or packager impact.
- Do not add dependencies for formatting, trivial parsing, or code that the standard library already handles clearly.

## Required Local Checks

Run these before handing off a PR:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all --all-features
./scripts/smoke-replay.sh
```

The smoke replay must use `examples/traces/kvm_exit_sample.log`.

## Writing Rules

- State what is implemented, what is tested, what is unsupported, and what still fails.
- Do not use marketing language, filler, or unsupported claims.
- Do not add fake benchmarks, fake compatibility matrices, fake hardware support, or invented test results.
- Do not leave task-marker placeholders, angle-bracket placeholders, or handoff text that pushes required work to the reader in committed files.
