# Detector engine

AegisHV now has a detector engine library layer. It is separate from the trace ingestion hot path. Existing runtime behavior is unchanged unless a caller wires this layer into a runtime path.

## Implemented pieces

- `Detector` trait with typed clean, finding, degraded, unsupported, and error outcomes.
- `DetectorScheduler` with per-detector enable flags.
- Per-detector runtime and finding-count budgets.
- Budget accounting for over-budget, truncated, unsupported, and degraded runs.
- Severity and confidence scoring from source reliability, attribution quality, profile confidence, identity confidence, data loss, and policy match.
- Normalizers for Linux and Windows kernel text hash drift.
- Normalizers for Linux syscall table/LSTAR and Windows SSDT/LSTAR drift.
- Inventory comparison detectors for hidden process and hidden module or driver candidates.
- Executable anonymous memory and RWX mapping detectors.
- JIT allowlist model for expected executable anonymous mappings.
- W^X event bridge that preserves guest process, symbol, and page attribution already present on the event.
- Dedupe and aggregation keyed by detector, VM, entity, range, and symbol.
- Incident object model for VM-local correlation of W^X, syscall hook evidence, and kernel text hash drift.
- Versioned detector state file for dedupe and incident summaries.

## Boundaries

The detector engine is not a live guest backend. It does not read guest memory, registers, page tables, kernel profiles, or process lists by itself.

Current detector inputs must come from existing trace events, synthetic fixtures, or caller-supplied offline VMI reports. Unsupported inputs return typed unsupported or degraded outcomes. The scheduler records those outcomes instead of reporting clean success.

No public event schema changed in this layer. Incident records are Rust objects, not emitted JSONL events in the current runtime.

## Budgets

The scheduler checks elapsed runtime after each detector returns and records over-budget status. It also truncates findings beyond the configured finding budget. It does not preempt a running detector thread. A future runtime integration would need a worker boundary before hard timeout enforcement can be claimed.
