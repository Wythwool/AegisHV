# Event Export Design

This document describes a possible OTLP export path for AegisHV. It is design-only. The current runtime does not send OTLP logs or metrics, does not validate against an OpenTelemetry collector, and does not implement SIEM, OCSF, or ECS output.

JSONL remains the primary event stream. Syslog and journald are optional mirrors of the JSON event line. A later OTLP sink should preserve that ordering and failure model instead of replacing JSONL.

## Scope

The intended boundary is the `EventSink` abstraction. Event construction, W^X correlation, policy evaluation, lifecycle events, and action audit events should continue to produce the existing AegisHV event object. The OTLP sink should serialize an already-built event into an OTLP log record.

Out of scope for this design:

- changing `schema/event.schema.json`;
- changing replay or deterministic replay output;
- adding OpenTelemetry dependencies to the main crate without dependency review;
- adding backend, type-1, VMI, EPT/NPT, syscall integrity, or hardware PMU claims;
- exporting raw XML, command lines, socket paths, host paths, VM-name-derived labels, arbitrary error strings, or secrets as labels or resource attributes.

## Proposed Configuration Shape

This section is not implemented. It records the shape a runtime implementation should validate before enabling OTLP.

```toml
[otlp]
enable = false
protocol = "http/protobuf"
endpoint = "http://127.0.0.1:4318/v1/logs"
timeout_ms = 2000
batch_max_events = 256
batch_max_bytes = 1048576
queue_capacity = 8192
retry_max_attempts = 3
retry_initial_ms = 250
retry_max_ms = 5000
tls_ca_file = ""
headers = []
```

Validation rules should be strict:

- `enable=false` preserves current behavior.
- `protocol` is a bounded enum such as `http/protobuf` or `grpc`.
- `endpoint` must parse as a URL for HTTP or an authority for gRPC.
- timeouts, batch size, byte limits, queue capacity, and retry values must have documented min/max bounds.
- `headers` must reject duplicate names, control characters, and empty names.
- header values must never be emitted in logs, metrics labels, lifecycle events, action audit details, or health responses.

Changing OTLP settings should be startup-only unless the implementation proves reload semantics, drains old queues safely, and emits explicit lifecycle diagnostics for reload failures.

## Resource Attributes

Resource attributes identify the AegisHV sensor process and deployment context. They must be bounded and must not contain tenant secrets or unbounded host paths.

Recommended attributes:

| Attribute | Value source | Notes |
| --- | --- | --- |
| `service.name` | fixed `aegishv` | bounded |
| `service.version` | crate version | bounded |
| `aegishv.schema_version` | event schema version | bounded integer |
| `aegishv.sensor_id` | existing sensor metadata when present | omit when unavailable |
| `host.name` | existing host metadata when present | omit when unavailable |
| `aegishv.runtime.mode` | `tracefs` or `replay` | lifecycle-derived, bounded |
| `aegishv.backend` | fixed `host_tracefs_sensor` | must not claim type-1 or VMI |

Do not export raw config paths, QMP sockets, dump paths, spool paths, tracefs paths, raw command lines, XML, or arbitrary errors as resource attributes.

## Log Record Mapping

Each emitted AegisHV event maps to one OTLP log record.

| AegisHV field | OTLP field |
| --- | --- |
| `ts` | `time_unix_nano` parsed from RFC3339 when possible |
| `monotonic_ms` | attribute `aegishv.monotonic_ms` |
| `sequence` | attribute `aegishv.sequence` |
| `event_id` | `trace_id` is not derived from it; store as `aegishv.event_id` |
| `category` | `event.name` prefix or attribute `aegishv.category` |
| `severity` | OTLP severity number and text through a fixed mapping |
| full JSON event | log body string or structured body map |

The first implementation should prefer a string body containing the exact JSON event line. That keeps schema validation and replay comparison anchored to the existing event contract. A structured body can be added later only if it preserves every existing event field and has tests for null handling.

Severity mapping:

| AegisHV severity | OTLP severity number | OTLP severity text |
| --- | --- | --- |
| `critical` | 21 | `FATAL` |
| `high` | 17 | `ERROR` |
| `medium` | 13 | `WARN` |
| `low` | 9 | `INFO` |
| `info` | 9 | `INFO` |

The mapping is intentionally coarse. It must stay bounded and must not derive severity from free-form message text.

## Event Attributes

The OTLP sink may add small fixed attributes for indexing:

- `aegishv.category`
- `aegishv.severity`
- `aegishv.reason`
- `aegishv.sequence`
- `aegishv.schema_version`
- `aegishv.vm_id_present`
- `aegishv.identity.confidence`
- `aegishv.identity.ambiguous`
- `aegishv.loss.data_loss`
- `aegishv.action.kind`
- `aegishv.action.result`
- `aegishv.action.failure_class`

Do not add VM names, UUIDs, PIDs, TIDs, socket paths, host paths, command lines, XML, raw trace text, or arbitrary error strings as OTLP attributes. If those values are already present in the JSON event body, they remain governed by the JSONL event schema and current event semantics. They still must not become metric labels or OTLP resource attributes.

## Identity Metadata

Identity metadata should be exported exactly as bounded event fields:

- `identity.sources`: bounded strings such as `trace_comm`, `proc_cmdline`, `proc_cgroup`, `libvirt_xml`, `libvirt_lifecycle`, `qmp_socket_hint`, `fallback_pid`, `ambiguous`, and `start_time_verified`;
- `identity.confidence`: `low`, `medium`, or `high`;
- `identity.start_time_verified`: boolean;
- `identity.ambiguous`: boolean.

QMP safety must not rely on OTLP export. Action safety remains a runtime policy decision before socket selection. OTLP only reports the already-decided result.

## Loss Semantics

Loss data must preserve the current distinction between aggregate counters and known emitted-sequence gaps.

When `data_loss=true`, export the existing `loss` object in the JSON body and fixed attributes:

- `aegishv.loss.data_loss=true`
- `aegishv.loss.reason`
- `aegishv.loss.range_kind`

Only export `aegishv.loss.sequence_gap_start` and `aegishv.loss.sequence_gap_end` when the event already contains exact values. Do not invent loss ranges from aggregate queue counters or exporter retry failure counts.

Exporter-side drops should be reported separately from telemetry-source drops. A later implementation should emit a `sensor` event with a bounded reason such as `otlp_export_loss` and increment bounded metrics such as `aegishv_otlp_write_failures_total` and `aegishv_otlp_dropped_total`.

## Lifecycle Events

Lifecycle events should be exported as ordinary `sensor` events. Do not synthesize additional OTLP-only lifecycle records unless the exporter has its own state transition that is not visible in JSONL.

Startup should report OTLP configuration in bounded terms only:

- enabled or disabled;
- protocol enum;
- endpoint class such as `local_http`, `remote_http`, `grpc`, or `invalid`;
- batch and queue limits.

It must not log full endpoints with credentials, headers, socket paths, or CA file paths. Shutdown should include bounded exporter counters if implemented.

## Action Audit Fields

Action audit events already carry the fields needed for OTLP export:

- `action.kind`
- `action.decision`
- `action.status`
- `action.result`
- `action.failure_class`
- `action.attempt`
- `action.max_attempts`
- `action.retry_count`
- `action.timeout_ms`
- `action.timed_out`
- `action.refused`

These map to attributes only when the value is bounded. `action.detail` is operator text and must remain in the JSON body only. Do not turn it into an OTLP attribute or metric label.

## Metrics Mapping

The existing Prometheus text endpoint remains the implemented metrics interface. OTLP metrics export is not implemented.

If OTLP metrics are added later, use the same bounded series as `src/metrics.rs`. Safe examples:

- ingest, parse, malformed, unsupported, and trace-input reason counters;
- event counters by bounded `category` and `severity`;
- W^X counters and tracked-page gauge;
- policy counters using existing bounded rule IDs only if rule IDs stay operator-controlled and bounded;
- identity cache/confidence/conflict counters with existing bounded labels;
- output failure counters for JSONL, spool, syslog, journald, and OTLP.

Do not use VM names, UUIDs, PIDs, TIDs, socket paths, host paths, raw errors, raw XML, command lines, or trace text as metric labels.

## Batching And Backpressure

The exporter should not block parser, W^X, or action hot paths on network I/O. A bounded queue should sit behind the `EventSink` path.

Recommended behavior:

1. JSONL write or spool handling happens first.
2. The OTLP sink receives the serialized event line.
3. If the exporter queue has capacity, enqueue the event for batching.
4. If the exporter queue is full, count the exporter drop, mark output degraded, and emit a bounded loss diagnostic when possible.
5. A background worker batches by count, byte size, or flush interval.
6. The worker applies bounded retries with jitter-free deterministic tests.
7. After retry exhaustion, drop the batch, count it, and emit a bounded diagnostic through the normal event path.

Exporter queue overflow is not the same as source telemetry queue overflow. Source telemetry loss reports what the collector/runtime lost before event emission. Exporter loss reports that a secondary mirror failed after JSONL handling.

## Failure Behavior

Startup failures:

- invalid config fails startup;
- unsupported protocol fails startup;
- TLS material parse failure fails startup if TLS is required;
- endpoint connection should not be required at startup unless explicitly configured as `require_startup_connect=true`.

Runtime failures:

- failed sends increment OTLP write failure counters;
- retry exhaustion increments OTLP dropped counters;
- output health is degraded while JSONL remains healthy and OTLP is losing mirrored events;
- output health is failed only if the sink contract requires all enabled sinks to succeed before returning success.

The implementation must choose and document one of two modes:

- `best_effort_mirror`: JSONL success is enough for runtime success; OTLP failures degrade readiness but do not fail the process.
- `required_mirror`: OTLP failure is fatal after retry exhaustion.

The default should be `best_effort_mirror` to match the current optional mirror model used by syslog and journald only if the implementation also proves explicit counters, health degradation, and bounded loss diagnostics.

## Security Considerations

OTLP export can cross host boundaries. Treat it as a security-sensitive output path.

Requirements:

- disable by default;
- redact or omit configured headers in all logs and events;
- support TLS verification before any insecure mode;
- make insecure transport opt-in with a loud config name such as `allow_insecure = true`;
- bound queue memory and batch size;
- reject unbounded attributes;
- do not export dump contents or guest memory;
- do not claim guest integrity from telemetry export;
- keep action refusal details bounded in attributes;
- preserve existing QMP identity safety before export.

## Test Plan For Implementation

A runtime implementation should add tests for:

- config accept/reject cases for protocol, endpoint, bounds, headers, and TLS settings;
- OTLP log record mapping for every event category;
- severity mapping;
- identity metadata mapping with low, medium, high, ambiguous, stale, and conflicting cases;
- loss object mapping for aggregate counters and sequence gaps;
- lifecycle startup/shutdown export fields without secrets;
- action audit export for decision, refusal, timeout, retry, and failure classes;
- exporter queue full behavior and bounded counters;
- retry exhaustion and degraded health;
- deterministic replay output unchanged when OTLP is disabled;
- schema validation of JSONL output unchanged when OTLP is enabled;
- no high-cardinality labels or attributes from VM names, UUIDs, sockets, paths, command lines, XML, or raw errors.

Normal tests must not require a live OpenTelemetry collector. Use a mock HTTP/gRPC receiver or test writer that records exported batches and can inject send failures.

## Current Status

No OTLP runtime code exists in this tree. No OTLP config section is accepted by `config.example.toml` or the runtime parser. This document is a design note for a later implementation and does not change AegisHV output behavior.
