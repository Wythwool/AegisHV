# Architecture

AegisHV currently runs as a Linux host-side sensor. The active data path is intentionally simple:

1. `collector` reads live tracefs `trace_pipe` or a replay file.
2. Control messages such as replay EOF and collector errors travel on a separate channel so they cannot be dropped with telemetry.
3. `parser` classifies each raw line as parsed `kvm_exit`, unrelated tracepoint, unsupported tracepoint, or malformed `kvm_exit`.
4. `identity` enriches events with best-effort VM identity from `/proc`, PID start time, QEMU args, cgroups, and UUID/name hints.
5. `wx` correlates write then execute activity on the same VM/address-space/page.
6. `policy` evaluates rules and calls `actions` for QMP-backed response.
7. `event` serializes JSONL; `metrics` exposes Prometheus-compatible text.

## Active ingestion

The active collector is still text `trace_pipe`. `src/trace_format.rs` parses `events/*/*/format` files and is kept as the boundary for a future binary/perf/ring-buffer collector, but it is not yet wired into the runtime ingestion path.

## Identity model

The preferred event identity is stable `vm_id`:

- `libvirt:<uuid>` when a QEMU UUID is visible;
- `name:<domain>` when a domain name is visible;
- `host-pid:<pid>:start:<ticks>` as a fallback with PID reuse defense.

Events also carry a nullable `identity` object. `identity.sources` is a bounded source stack such as `trace_comm`, `proc_cmdline`, `proc_cgroup`, `libvirt_xml`, `libvirt_lifecycle`, `qmp_socket_hint`, `fallback_pid`, `ambiguous`, and `start_time_verified`. `identity.confidence` is `low`, `medium`, or `high`; PID-only and trace-comm identities stay low. High confidence requires libvirt UUID metadata verified against observed PID/TID start-time ticks.

QMP actions require `vm_id` mappings by default. VM-name fallback is only used when `identity.require_stable_qmp_match=false`.

## Loss model

Telemetry queue overflow is not silent. Dropped telemetry increments metrics, and the next emitted event receives:

```json
"data_loss": true,
"loss": {
  "dropped_since_last_event": 12,
  "dropped_total": 12,
  "reason": "queue_full_or_output_backpressure",
  "range_kind": "aggregate_counter",
  "sequence_gap_start": null,
  "sequence_gap_end": null
}
```

Queue overflow happens before a trace line becomes an event, so AegisHV does not invent event sequence numbers for those drops. `range_kind=aggregate_counter` means the count is known but the exact event range is not. If the output pipeline observes an emitted event sequence discontinuity, `range_kind=sequence_gap` reports the bounded `sequence_gap_start` and `sequence_gap_end` values on the next emitted event.

## Backend boundaries

`src/hypervisor.rs` and `src/vmi.rs` define the future separation between this host-side sensor and a live VMI/trap backend. `NoHypervisorBackend` reports `BackendArch::None`; it does not pretend to be Intel VMX.

The separate `aegishv-type1-kernel` target is a bootable x86_64 lab path, not a backend for the userspace sensor. It wires one BSP-only Intel VMX toy guest, while the production host paging, SMP, interrupt, device, lifecycle, and hardware-evidence gates remain open.

## Architecture decisions

Architecture Decision Records live in `docs/adr/README.md`. Use an ADR when a change affects a long-lived boundary, event contract, backend contract, dependency policy, security posture, or release rule.
