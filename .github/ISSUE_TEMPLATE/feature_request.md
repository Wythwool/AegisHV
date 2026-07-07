---
name: Feature request
about: Propose a bounded change to current AegisHV behavior.
title: "feature: "
labels: enhancement
---

## Summary

Describe the user-facing or operator-facing behavior being requested.

## Scope

State the smallest useful change. Include affected files or modules when known.

## Non-Goals

List behavior this request must not change.

## Acceptance Criteria

List the observable behavior and tests that would close this request.

## Schema Impact

State whether `schema/event.schema.json` or `schema/snapshot.schema.json` would change. Schema changes must include schemas, tests, docs, examples, and compatibility notes.

## Security Impact

State whether this touches identity, actions, policy, backend contracts, trap logic, guest memory, or operator permissions.

## Unsupported Claims Check

Do not describe this request as type-1 support, full VMI support, EPT/NPT/Stage-2 enforcement, syscall-path integrity, hardware PMU sampling, libvirt support, or release qualification unless the request includes the code and tests needed for that claim.
