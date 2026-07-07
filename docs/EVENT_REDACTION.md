# Event Redaction Policy

This document defines how AegisHV event data should be treated by operators and by later sink/export implementations. Runtime redaction is not implemented in the current tree. AegisHV does not provide a privacy guarantee for JSONL, syslog, journald, spool, metrics, snapshots, or design-only exporters.

The current runtime reduces some exposure by bounding metric labels, omitting raw XML, avoiding QMP socket paths in events, and keeping tracefs diagnostics structured. That is not a general redaction engine. Operators must treat emitted event files and mirrored logs as sensitive telemetry.

## Scope

This policy covers:

- JSONL event output;
- UDP syslog mirroring;
- Linux journald mirroring;
- disk spool segments, compressed or plaintext;
- OTLP export design;
- OCSF/ECS mapping docs and downstream transforms;
- action audit fields;
- identity metadata;
- VM inventory snapshots;
- tracefs diagnostics;
- lifecycle events;
- loss events and sequence-gap diagnostics;
- metrics label rules.

It does not change runtime behavior, schemas, replay output, deterministic fixtures, action safety, or sink failure handling.

## Sensitive Event Fields

The following fields can identify hosts, VMs, processes, guests, or operator policy. Treat them as sensitive when exporting outside the local trust boundary.

| Field or group | Why it may be sensitive | Current handling |
| --- | --- | --- |
| `host_id`, `sensor_id`, `tenant_id` | deployment identity and tenant routing | emitted when configured or discovered |
| `vm`, `vm_id`, `vm_name` | VM identity, UUID-derived stable IDs, or fallback PID identity | emitted in JSONL when known |
| `raw_comm` | host trace command text and PID-derived process hints | emitted from trace text when parsed |
| `host_pid`, `host_tid`, `host_start_time_ticks` | host process identity and PID-reuse disambiguation | emitted when known |
| `vcpu_id`, `vcpu`, `host_cpu` | host/guest execution placement | emitted when known |
| `arch`, `cr3`, `asid`, `vmid`, `vpid` | address-space metadata | emitted when parsed |
| `addr.rip`, `addr.gva`, `addr.gpa`, `addr.qual` | guest address and fault context | emitted for parsed exits |
| `violation`, `page_permissions_before`, `page_permissions_after`, `wx` | memory-access and W^X correlation evidence | emitted for matching events |
| `pmu.thread`, `pmu.pid`, `pmu.tid` | host target thread identity | emitted by PMU fallback heartbeat |
| `rule_id`, `decision`, `action_id`, `action_status`, `action.*` | policy and response behavior | emitted for policy/action events |
| `action.detail` | operator text about refusal, timeout, failure, or dry run | emitted as event body text only |
| `message` | operator diagnostic text | emitted for lifecycle, tracefs, loss, and diagnostics |
| `tags` | bounded state hints such as identity conflict tags | emitted when set |
| `loss` | queue or emitted-sequence loss metadata | emitted when loss is known |

These fields are useful for operations. They are not safe metric labels unless this policy explicitly says they are.

## Intentionally Excluded Data

Current event and snapshot behavior is intended to avoid these raw values:

- raw libvirt XML;
- raw QEMU command lines;
- QMP socket paths;
- dump output paths;
- tracefs root paths in event JSONL;
- disk spool paths in event JSONL;
- arbitrary backend error strings in metric labels;
- configured syslog or journald destinations in metric labels;
- secrets, private keys, tokens, passwords, OTLP headers, TLS material, or environment variables;
- guest memory contents;
- memory dump contents.

Snapshot JSON still includes `tracefs_root` and `trace_pipe` fields by schema. Operators should treat snapshots as local diagnostic artifacts. If snapshots are exported later, path fields need an explicit redaction mode or a documented decision to omit them from indexed/exported attributes.

## Metrics Label Rules

Metrics labels must stay bounded. They must not be derived from:

- raw trace text;
- VM names;
- UUIDs;
- PIDs or TIDs;
- socket paths;
- host paths;
- command lines;
- XML;
- event messages;
- `action.detail`;
- arbitrary error strings;
- tenant secrets or environment values.

Safe label sources are fixed enums or small code-controlled sets, such as event category, severity, trace input reason, bounded identity confidence, bounded conflict reason, sink name, component state, and bounded action failure class.

If a later metric needs a value outside a bounded enum, expose it as event body text or a count without labels. Do not add a hash label as a shortcut; hashes of VM names, paths, or UUIDs are still high-cardinality identifiers.

## Sink Policy

### JSONL

JSONL is the primary event stream and currently receives the full event object. Runtime redaction is not implemented. Protect JSONL files with filesystem permissions, retention policy, and log shipping controls. Do not assume replay mode or deterministic replay removes all sensitive fields unless the fixture was generated specifically for that purpose.

A later redaction implementation should run before `EventSink` fan-out and before signing, if event signing is added. It must document whether redaction changes the signed bytes or whether signatures apply only to unredacted local JSONL.

### Syslog

Syslog mirrors the JSON event line through UDP when enabled. It should be treated as a potentially remote disclosure path. The syslog sink must not create labels from VM names, UUIDs, socket paths, paths, command lines, XML, or raw errors. If a later implementation adds syslog-specific redaction, failure to redact must be explicit and counted.

### Journald

Journald mirrors the JSON event line to a local datagram socket when enabled. `MESSAGE` contains the event JSON line. Structured fields are bounded to fixed AegisHV keys today. Do not add VM identity, socket path, host path, command-line, XML, or arbitrary error fields as journald indexed fields.

### Disk Spool

The spool preserves event lines that could not be written to the main JSONL output. Plaintext v1 and RLE-compressed v2 segments both contain the same event data after decoding. Compression is not redaction or encryption. Treat spool directories as sensitive and do not rely on segment compression to hide content.

If redaction is added later, spool should receive the same redacted line as JSONL unless the operator explicitly configures an unredacted local spool. That exception would need separate documentation and tests.

### OTLP Design

OTLP export is design-only in this tree. Any later OTLP sink should keep high-cardinality and sensitive values in the event body only when they are already part of JSONL semantics. Resource attributes and log attributes must stay bounded. Do not export headers, TLS material, endpoints with credentials, socket paths, host paths, VM-name-derived labels, UUID-derived labels, command lines, XML, or raw errors.

### OCSF/ECS Mapping Docs

The OCSF/ECS mapping notes are documentation only. Downstream transforms should keep AegisHV JSON as the source of truth, index only bounded fields, and leave operator text as text. Do not map raw XML, command lines, socket paths, dump paths, tracefs paths, host paths, arbitrary errors, secrets, or VM-name-derived values into indexed fields.

## Event-Type Policy

### Action Audit Details

Action audit events intentionally expose decision, attempt, result, timeout, refusal, retry, and bounded failure-class metadata. `action.detail` is operator text. It may describe why an action was refused or failed. It must not become a metric label, OTLP attribute, OCSF indexed field, ECS keyword, journald structured field, or syslog routing key.

Unsafe dump paths, QMP failures, stable-identity mismatches, and unsupported actions should use bounded failure classes. Raw paths, socket names, VM names, raw QMP responses, and arbitrary backend errors should not be copied into label-like fields.

### Identity Metadata

Identity metadata uses bounded sources and confidence values, but identity values themselves can be sensitive. `identity.sources`, `identity.confidence`, `identity.start_time_verified`, and `identity.ambiguous` are safe for bounded indexing. `vm`, `vm_id`, `vm_name`, host PID/TID, and start-time ticks are not safe metric labels.

Identity confidence is an attribution signal. It is not a privacy classification and not proof of guest integrity.

### VM Inventory

Snapshot `vm_inventory` can contain VM UUID/name fields, host task IDs, vCPU mappings, QMP presence status, identity confidence/source information, and conflict state. The snapshot schema excludes raw XML, raw command lines, socket paths, and host paths from VM entries. VM UUID/name fields are still sensitive.

Future exporters should either omit VM UUID/name from indexed attributes or require an explicit operator-controlled allowlist. Conflict reasons such as `stale_cache`, `pid_reuse`, and `qmp_socket_mismatch` are bounded and can be indexed.

### Tracefs Diagnostics

Tracefs diagnostics report bounded system/name/status/missing-field data and operator text about missing or malformed tracepoint metadata. They should not include raw tracefs format files or full tracefs paths in event JSONL. Missing field names are bounded by expected parser groups and are safe for indexing when kept to known strings.

### Lifecycle Events

Lifecycle events summarize startup, shutdown, signal handling, and reload behavior. They intentionally avoid configured paths and unsupported backend claims. Their `message` fields can still reveal mode, enabled sinks, queue size, policy counts, QMP mapping counts, identity settings, PMU fallback state, and output health counters. Treat lifecycle messages as operational telemetry, not public status banners.

### Loss Events

Loss events expose dropped counts, total counts, bounded reason, range kind, and known sequence-gap ranges. These fields are safe for indexing when reason and range kind remain bounded. Loss events must not carry raw dropped trace text or invented details about missing events.

## Future Redaction Implementation Requirements

A runtime redaction implementation should be explicit and testable:

- disabled by default unless a config file opts in;
- config validates field names against a fixed allowlist;
- redaction runs before sink fan-out;
- deterministic replay behavior is documented;
- signed event behavior is documented if event signing exists;
- schema changes, examples, docs, and compatibility notes land in the same change;
- metrics count redaction failures with bounded reasons;
- refusal or failure behavior is explicit when a required redaction rule cannot run;
- tests cover JSONL, syslog, journald, spool, replay, snapshot/export examples, action audit, identity, tracefs diagnostics, lifecycle, and loss events.

Redaction must not silently replace an unsafe action failure with success, weaken action identity safety, or remove fields needed to detect data loss.

## Operator Guidance

- Store JSONL and spool under restricted ownership.
- Treat syslog and journald mirrors as carrying the same sensitive event body as JSONL.
- Do not expose `/metrics` outside the intended local monitoring boundary.
- Validate downstream transforms so they do not index VM names, UUIDs, paths, command lines, XML, or raw errors.
- Review retention for JSONL, spool, snapshots, and mirrored logs together.
- Do not ship memory dumps through event sinks. Dump handling has separate path checks and separate storage risk.

## Current Status

Runtime event redaction is not implemented. No `[redaction]` config section exists. No schema fields declare redaction state. No sink rewrites or strips event fields today. This policy documents what must be protected now and how later redaction work should be bounded and tested.
