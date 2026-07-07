---
name: Detector task
about: Track parser, W^X, policy, PMU fallback, or detection rule work.
title: "detector: "
labels: detector
---

## Summary

Describe the signal, pattern, rule, or detector behavior.

## Data Source

State the tracefs line, replay fixture, event category, policy rule, metric, or backend signal used by the detector.

## Scope

State what changes in parsing, correlation, policy matching, metrics, or event output.

## Acceptance Criteria

Include tests for detection, non-detection, malformed input, unsupported input, and degraded telemetry when relevant.

## False-Positive Impact

Describe benign activity that could trigger the detector and how it is bounded.

## False-Negative Impact

Describe activity the detector still misses after this task.

## Schema Impact

State whether event or snapshot schemas change. If they do, include migration work in the same task.
