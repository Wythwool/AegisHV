# Downstream event mapping

AegisHV emits its own JSON schema. OCSF/ECS conversion belongs in a downstream
collector; no such exporter ships here.

Keep `ts` as wall time and `monotonic_ms` as process-local elapsed time.
`sequence` orders emitted events. `host_cpu` comes from the Linux trace header;
`vcpu_id` is a guest vCPU only when the tracepoint supplies one. `vcpu` is the
compatibility alias for `vcpu_id`.

Do not coerce unavailable values to zero. Keep `loss`, identity confidence,
ambiguity, and source fields. Guest and host process identifiers are separate.
Addresses remain strings so downstream numeric conversions cannot lose bits.

Categories are `exit`, `wx`, `pmu`, `policy`, and `sensor`. Use the action audit
fields for response results: `accepted`, `dry_run`, `refused`, and completed
actions have different meanings. A W^X alert describes observed correlation,
not a denied memory access.

VM names, paths, addresses, messages, and action details can have unbounded
cardinality. Keep them in event bodies rather than metric labels.
