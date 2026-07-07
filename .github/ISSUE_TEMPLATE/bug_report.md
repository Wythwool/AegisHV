---
name: Bug report
about: Report a reproducible defect in current AegisHV behavior.
title: "bug: "
labels: bug
---

## Summary

State the incorrect behavior and the expected behavior.

## Affected Area

- Host-side tracefs sensor
- Replay parser
- W^X correlation
- Policy or QMP action
- Metrics or health endpoint
- Packaging or service file
- Documentation

## Reproduction

List the exact command, config, replay file, trace line, or service action that reproduces the issue.

## Observed Result

Include relevant logs, JSONL events, metrics, or error messages.

## Expected Result

State the behavior AegisHV should have for this input.

## Safety Boundary

Confirm whether this report touches identity, QMP actions, VMI contracts, type-1 backend contracts, schema output, or security-sensitive refusal behavior.

## Tests Run

List the local commands run and their result. If a command could not run, state the missing tool or host requirement.
