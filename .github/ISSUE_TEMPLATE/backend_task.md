---
name: Backend task
about: Track work on VMI, trap, hypervisor, or no-backend contracts.
title: "backend: "
labels: backend
---

## Summary

Describe the backend boundary or runtime capability under discussion.

## Current State

State what the current code does. If the behavior is unsupported, say so directly.

## Scope

Name the contract, backend, architecture, or refusal path affected by this task.

## Acceptance Criteria

Include positive and negative tests. Cover unsupported backend, ambiguous identity, unsafe input, partial capability, and refusal behavior when relevant.

## Production Gate

State the evidence required before this can be described as supported behavior. Hardware-backed claims need hardware-backed tests.

## Safety Boundary

Do not mark a backend path as accepted unless the implementation verifies the requested capability and returns a typed error when it cannot.
