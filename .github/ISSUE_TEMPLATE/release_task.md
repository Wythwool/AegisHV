---
name: Release task
about: Track release qualification, packaging, and publication work.
title: "release: "
labels: release
---

## Summary

Name the release or release-candidate action.

## Scope

List the artifacts, tags, workflows, packages, docs, and validation steps included in this release task.

## Required Checks

- `cargo fmt --all -- --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all --all-features`
- `./scripts/smoke-replay.sh`
- package contents check
- schema validation check

## Acceptance Criteria

Record the exact commands run, their results, and artifact paths.

## Security Impact

State whether the release changes service permissions, action behavior, default config, backend boundaries, or operator-visible failure behavior.

## Production Claim Check

Do not claim support for type-1, VMI, EPT/NPT/Stage-2 enforcement, syscall-path integrity, hardware PMU sampling, libvirt, or hardware coverage unless the release contains that implementation and test evidence.
