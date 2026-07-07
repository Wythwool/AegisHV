# ADR-0001: Record Architecture Decisions as ADRs

## Status

Accepted on 2026-04-24.

## Context

AegisHV currently has an active Linux host-side KVM sensor and explicit boundaries for VMI and type-1 backend work. Some decisions affect more than one file or release, especially event contracts, backend contracts, security behavior, dependency policy, and operator-visible behavior.

Those decisions need a stable place outside commit messages and pull request discussion. The record must not imply support for type-1, full VMI, EPT/NPT/Stage-2 enforcement, syscall-path integrity, hardware PMU sampling, libvirt integration, or hardware coverage unless code and tests are present.

## Decision

Record long-lived architecture decisions in `docs/adr/`.

Use a numbered file name:

```text
NNNN-short-title.md
```

Each ADR must include:

- Status
- Context
- Decision
- Consequences
- Test Impact

The ADR index in `docs/adr/README.md` lists each record by number, title, and status.

Valid statuses are:

- Proposed
- Accepted
- Superseded
- Rejected

An ADR records the decision that was made. It must not be used to claim support that the runtime does not implement and test.

## Consequences

Architecture changes have a stable review trail.

Reviewers can check whether a change affects existing contracts before reading implementation details.

The cost is a small amount of documentation whenever a change crosses a long-lived boundary.

## Test Impact

Changes to ADR structure are covered by `tests/adr_tests.rs`.

An ADR that changes runtime behavior, event schemas, backend contracts, security behavior, or operator-visible behavior must name the tests and docs that prove the change.

