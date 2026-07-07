# Event Field Mapping Notes

This document maps the current AegisHV JSONL event fields to candidate OCSF and ECS-style fields for a future exporter or downstream transform. It is documentation only. AegisHV does not currently emit OCSF, ECS, SIEM-normalized, or production telemetry-normalized output.

JSONL remains the contract. Any future transform must validate against the target OCSF or ECS version it claims to support. The field names below are a design guide, not proof of compatibility with a specific collector, SIEM, or schema version.

## Mapping Rules

- Preserve the original AegisHV JSON event as the source of truth.
- Keep indexed fields bounded. Prefer booleans, enums, integers, and fixed strings already present in the event contract.
- Keep operator text in text-only fields. Do not turn it into metric labels, OCSF indexed fields, ECS keywords, or OTLP attributes without an explicit bounded allowlist.
- Do not map raw XML, raw command lines, QMP socket paths, dump paths, tracefs paths, host paths, arbitrary error strings, secrets, or VM-name-derived labels into indexed fields.
- Do not infer unsupported backend capabilities. AegisHV is still a host-side KVM tracefs sensor.

## Event Class Selection

The current `category` drives the downstream event class. A transform should keep the original `aegishv.category` regardless of any target class.

| AegisHV category | Typical event | OCSF-style target | ECS-style target | Notes |
| --- | --- | --- | --- | --- |
| `exit` | parsed KVM exit or stage-2 fault text | kernel, virtualization, or security finding class chosen by the downstream owner | `event.category: host`, `event.type: info` or `event.type: denied` | The current event is tracefs telemetry, not enforcement proof. |
| `wx` | correlated write-then-execute page activity | finding or detection event | `event.kind: alert`, `event.category: threat`, `event.type: indicator` | This is correlation only, not EPT/NPT enforcement. |
| `pmu` | PMU fallback heartbeat sample | host/process metric-like event | `event.category: process`, `event.type: info` | Counters may be `null`; this is not true hardware PMU sampling. |
| `policy` | policy match or action audit | response/action result event | `event.category: configuration` or `event.category: threat`, `event.type: allowed`, `denied`, or `info` | Use bounded `action.*` fields. |
| `sensor` | lifecycle, diagnostics, loss, reload, identity conflict | system activity or health event | `event.kind: event`, `event.category: host`, `event.type: info`, `error`, or `change` | Reasons are bounded by runtime code. |

## Common Event Fields

| AegisHV field | OCSF-style field | ECS-style field | Indexed? | Notes |
| --- | --- | --- | --- | --- |
| `ts` | `time` | `@timestamp` | yes | Parse RFC3339; keep original on parse failure. |
| `monotonic_ms` | `metadata.aegishv.monotonic_ms` | `aegishv.monotonic_ms` | yes | Process-local ordering helper, not wall time. |
| `sequence` | `metadata.aegishv.sequence` | `event.sequence` | yes | Use for emitted-event ordering. |
| `event_id` | `metadata.uid` or `metadata.aegishv.event_id` | `event.id` | yes | Do not convert to a trace ID unless a real trace model exists. |
| `version` | `metadata.product.version` or `metadata.aegishv.event_version` | `aegishv.event.version` | yes | Event format version. |
| `schema_version` | `metadata.version` or `metadata.aegishv.schema_version` | `event.dataset` plus `aegishv.schema_version` | yes | Current JSON schema is the source contract. |
| `category` | `metadata.aegishv.category` | `event.category` plus `aegishv.category` | yes | Keep the original category. |
| `severity` | `severity` | `event.severity`, `log.level` | yes | Use a fixed severity mapping. |
| `reason` | `message` or `metadata.aegishv.reason` | `event.reason` | yes if bounded | Runtime reasons are bounded; free-form details are not. |
| `message` | `message` | `message` | no keyword | Operator text. Do not index as a label. |
| `tags` | `metadata.tags` | `tags` | yes only for bounded tags | Identity conflict tags are bounded. Reject arbitrary new tag sources. |
| `correlation_id` | `correlation_uid` | `event.grouping_key` | yes | Keep original string. |
| `rule_id` | `rule.uid` | `rule.id` | yes | Operator-controlled and should remain bounded. |
| `decision` | `disposition` or `metadata.aegishv.decision` | `event.action` | yes | Preserve exact runtime value. |
| `action_id` | `activity_id` or `metadata.aegishv.action_id` | `event.action` plus `aegishv.action_id` | yes | Runtime-generated bounded ID. |
| `action_status` | `status` | `event.outcome` | yes | Use only bounded status values. |

Recommended severity mapping:

| AegisHV severity | OCSF-style severity | ECS `event.severity` | ECS `log.level` |
| --- | --- | --- | --- |
| `critical` | `Critical` | `9` | `critical` |
| `high` | `High` | `7` | `error` |
| `medium` | `Medium` | `5` | `warning` |
| `low` | `Low` | `3` | `info` |
| `info` | `Informational` | `1` | `info` |

## Host, Process, And vCPU Fields

| AegisHV field | OCSF-style field | ECS-style field | Indexed? | Notes |
| --- | --- | --- | --- | --- |
| `host_id` | `device.uid` | `host.id` | yes | Existing host metadata only. |
| `sensor_id` | `metadata.product.uid` | `agent.id` | yes | Existing sensor metadata only. |
| `tenant_id` | `metadata.tenant_uid` | `organization.id` or `aegishv.tenant_id` | yes | Omit when unset. |
| `raw_comm` | `process.name` | `process.name` | no by default | Trace comm can be noisy; do not use as a label. |
| `host_pid` | `process.pid` | `process.pid` | yes | Host PID, not guest PID. |
| `host_tid` | `process.tid` | `process.thread.id` | yes | Host TID, not guest TID. |
| `host_start_time_ticks` | `process.created_time` alternative metadata | `aegishv.host_start_time_ticks` | yes | Linux `/proc` start-time ticks, not a wall timestamp. |
| `host_cpu` | `device.cpu.uid` or extension | `host.cpu.id` or `aegishv.host_cpu` | yes | Linux trace header CPU. |
| `vcpu_id` | `virtual_cpu.uid` extension | `aegishv.vcpu.id` | yes | Guest vCPU ID only when metadata exists. |
| `vcpu` | same as `vcpu_id` | same as `vcpu_id` | no duplicate | Backward-compatible alias. |
| `arch` | `device.arch` | `host.architecture` or `aegishv.arch` | yes | Architecture from trace parsing when present. |
| `cr3`, `asid`, `vmid`, `vpid` | virtualization address-space extension | `aegishv.address_space.*` | yes | Address-space identifiers; keep as strings. |
| `privilege_level` | `process.user_privileges` extension | `aegishv.privilege_level` | yes | Only when present. |
| `guest_os`, `guest_process`, `guest_thread`, `guest_module`, `guest_symbol` | guest context extension | `aegishv.guest.*` | no by default | Current runtime does not implement full guest OS/VMI resolution. |

Do not rename `host_cpu` to guest vCPU. The schema deliberately separates trace header CPU from `vcpu_id`.

## VM Identity Fields

| AegisHV field | OCSF-style field | ECS-style field | Indexed? | Notes |
| --- | --- | --- | --- | --- |
| `vm` | `vm.name` | `cloud.instance.name` or `aegishv.vm` | no by default | VM names can be high-cardinality and operator-defined. |
| `vm_id` | `vm.uid` | `cloud.instance.id` or `aegishv.vm_id` | yes only if downstream accepts stable IDs | Values can include a `libvirt:` UUID value, a `name:` domain value, or PID fallback. |
| `vm_name` | `vm.name` | `cloud.instance.name` or `aegishv.vm_name` | no by default | Keep out of labels unless bounded by downstream policy. |
| `identity.sources` | `metadata.aegishv.identity.sources` | `aegishv.identity.sources` | yes | Bounded enum strings. |
| `identity.confidence` | `metadata.aegishv.identity.confidence` | `aegishv.identity.confidence` | yes | `low`, `medium`, or `high`. |
| `identity.start_time_verified` | `metadata.aegishv.identity.start_time_verified` | `aegishv.identity.start_time_verified` | yes | Required for safe QMP action identity. |
| `identity.ambiguous` | `metadata.aegishv.identity.ambiguous` | `aegishv.identity.ambiguous` | yes | Ambiguous identity must stay visible. |

Identity confidence is not guest integrity. It only reports how the host-side sensor attributed an event to a VM.

## Address, Fault, And W^X Fields

| AegisHV field | OCSF-style field | ECS-style field | Indexed? | Notes |
| --- | --- | --- | --- | --- |
| `trap_type` | `activity_name` or virtualization extension | `aegishv.trap_type` | yes | Bounded by parser behavior. |
| `addr.rip` | `process.thread.instruction_pointer` extension | `aegishv.addr.rip` | yes | Guest instruction pointer when present. |
| `addr.gva` | virtualization memory extension | `aegishv.addr.gva` | yes | Guest virtual address when present. |
| `addr.gpa` | virtualization memory extension | `aegishv.addr.gpa` | yes | Guest physical address when present. |
| `addr.qual` | virtualization fault qualifier extension | `aegishv.addr.qual` | no by default | Hex qualifier; useful for debug. |
| `violation.read/write/exec` | access flags extension | `aegishv.violation.*` | yes | Decoded access bits. |
| `page_permissions_before` | memory permission extension | `aegishv.page_permissions_before.*` | yes | Current sensor reports correlation state, not enforcement changes. |
| `page_permissions_after` | memory permission extension | `aegishv.page_permissions_after.*` | yes | Do not claim EPT/NPT permission flips. |
| `wx.writer_rip` | finding evidence extension | `aegishv.wx.writer_rip` | yes | Writer instruction pointer. |
| `wx.executor_rip` | finding evidence extension | `aegishv.wx.executor_rip` | yes | Executor instruction pointer. |
| `wx.delta_ms` | `duration` or extension | `aegishv.wx.delta_ms` | yes | Correlation interval. |
| `wx.page_size` | memory extension | `aegishv.wx.page_size` | yes | Page size used for correlation. |
| `wx.confidence` | `confidence` | `event.risk_score` or `aegishv.wx.confidence` | yes | Correlation confidence, not enforcement proof. |

W^X events should map as detections or findings. They must not be mapped as blocked exploitation unless a future backend actually enforces memory permissions and tests that behavior.

## PMU Fallback Fields

| AegisHV field | OCSF-style field | ECS-style field | Indexed? | Notes |
| --- | --- | --- | --- | --- |
| `pmu.pid` | `process.pid` | `process.pid` | yes | Host PID. |
| `pmu.tid` | `process.tid` | `process.thread.id` | yes | Host TID. |
| `pmu.thread` | `process.name` | `process.thread.name` | no by default | Host thread name; can be noisy. |
| `pmu.cycles_delta` | metric extension | `aegishv.pmu.cycles_delta` | yes | Nullable. Null means unavailable. |
| `pmu.instr_delta` | metric extension | `aegishv.pmu.instr_delta` | yes | Nullable. |
| `pmu.cache_ref_delta` | metric extension | `aegishv.pmu.cache_ref_delta` | yes | Nullable. |
| `pmu.cache_miss_delta` | metric extension | `aegishv.pmu.cache_miss_delta` | yes | Nullable. |
| `pmu.branch_delta` | metric extension | `aegishv.pmu.branch_delta` | yes | Nullable. |
| `pmu.branch_miss_delta` | metric extension | `aegishv.pmu.branch_miss_delta` | yes | Nullable. |
| `pmu.sample_ms` | interval extension | `aegishv.pmu.sample_ms` | yes | Configured sample interval. |
| `pmu.source` | `metadata.aegishv.pmu.source` | `aegishv.pmu.source` | yes if bounded | Current fallback source only. |
| `pmu.grouped` | metric extension | `aegishv.pmu.grouped` | yes | False for current fallback. |

Do not map null PMU counters to zero. Zero would imply measured data that the current runtime does not have.

## Policy And Action Audit Fields

| AegisHV field | OCSF-style field | ECS-style field | Indexed? | Notes |
| --- | --- | --- | --- | --- |
| `action.rule` | `rule.name` or `rule.uid` | `rule.name` | yes if bounded | Operator-controlled. |
| `action.kind` | `activity_name` | `event.action` | yes | Fixed action kinds. |
| `action.ok` | `status` | `event.outcome` | yes | Boolean result summary. |
| `action.status` | `status_detail` | `aegishv.action.status` | yes | Bounded runtime value. |
| `action.decision` | `disposition` | `event.action` or `aegishv.action.decision` | yes | Decision before execution. |
| `action.result` | `status` | `event.outcome` | yes | `dry_run`, `completed`, `accepted`, `error`, `refused`, `timeout`, or `unsupported`. |
| `action.detail` | `message` | `message` or `error.message` | no keyword | Operator text. Do not index. |
| `action.latency_ms` | `duration` | `event.duration` converted to nanoseconds or `aegishv.action.latency_ms` | yes | Null when not measured. |
| `action.target_vm_id` | `vm.uid` | `cloud.instance.id` or `aegishv.action.target_vm_id` | yes only if downstream accepts stable IDs | Do not use VM names as labels. |
| `action.attempt` | retry extension | `aegishv.action.attempt` | yes | Bounded integer. |
| `action.max_attempts` | retry extension | `aegishv.action.max_attempts` | yes | Bounded integer. |
| `action.retry_count` | retry extension | `aegishv.action.retry_count` | yes | Bounded integer. |
| `action.timeout_ms` | timeout extension | `aegishv.action.timeout_ms` | yes | Configured timeout. |
| `action.timed_out` | timeout boolean extension | `aegishv.action.timed_out` | yes | Boolean. |
| `action.refused` | disposition extension | `aegishv.action.refused` | yes | Refusal before unsafe execution. |
| `action.failure_class` | `status_code` or extension | `error.type` or `aegishv.action.failure_class` | yes | Bounded enum. |

`dump_guest_memory` acceptance means QMP accepted the command. It does not prove the dump completed or that the file contents are safe.

## Loss And Range Fields

| AegisHV field | OCSF-style field | ECS-style field | Indexed? | Notes |
| --- | --- | --- | --- | --- |
| `data_loss` | `metadata.aegishv.data_loss` | `event.ingested` companion or `aegishv.data_loss` | yes | Boolean. |
| `loss.dropped_since_last_event` | loss extension | `aegishv.loss.dropped_since_last_event` | yes | Aggregate count. |
| `loss.dropped_total` | loss extension | `aegishv.loss.dropped_total` | yes | Aggregate count. |
| `loss.reason` | loss reason extension | `aegishv.loss.reason` | yes | Bounded runtime reason. |
| `loss.range_kind` | loss range extension | `aegishv.loss.range_kind` | yes | `aggregate_counter`, `sequence_gap`, or both. |
| `loss.sequence_gap_start` | loss range extension | `aegishv.loss.sequence_gap_start` | yes when present | Exact only when runtime knows it. |
| `loss.sequence_gap_end` | loss range extension | `aegishv.loss.sequence_gap_end` | yes when present | Exact only when runtime knows it. |

Do not invent exact loss ranges from aggregate queue counters, spool failures, syslog failures, journald failures, or future export failures.

## Lifecycle And Diagnostic Events

Lifecycle and diagnostic events are `sensor` events. A transform should keep `aegishv.reason` and map the downstream class from the bounded reason.

| AegisHV reason | OCSF-style target | ECS-style target | Notes |
| --- | --- | --- | --- |
| `sensor_startup` | system activity | `event.type: start` | Startup summary text is operator text. |
| `sensor_shutdown` | system activity | `event.type: end` | Includes bounded loss/output counters in message text. |
| `shutdown_signal` | system activity | `event.type: change` | Signal name is bounded by runtime. |
| `config_reload_failed` | configuration or health event | `event.type: error` | Do not index raw parser detail beyond bounded reason. |
| `config_reload_skipped` | configuration event | `event.type: info` | No config path should be indexed. |
| `jsonl_reopen_failed` | output health event | `event.type: error` | Do not index output path or raw error string. |
| `jsonl_reopen_skipped` | output health event | `event.type: info` | Stdout reopen skipped. |
| `telemetry_loss` | pipeline health event | `event.type: error` | Use `loss` object for indexed loss details. |
| `tracefs_format_diagnostic` | host diagnostic event | `event.type: info` or `error` | Field names and status are bounded; raw format text is not emitted. |
| `identity_conflict` | identity diagnostic event | `event.type: denied` or `error` | Use bounded conflict tags such as `identity_conflict:stale_cache`. |
| `libvirt_lifecycle` | VM lifecycle metadata event | `event.type: change` | Mockable metadata only; current binary does not subscribe to live libvirt. |

Tracefs diagnostics say whether expected metadata was present and parseable. They do not prove type-1 operation, full VMI, EPT/NPT enforcement, or binary perf ingestion.

## Export And Output Failure Fields

Current output failures are visible through JSONL sensor events and metrics. A downstream transform can map bounded failure counters but must not index raw error strings.

| Source | OCSF-style target | ECS-style target | Notes |
| --- | --- | --- | --- |
| `aegishv_json_write_failures_total` metric | pipeline health metric | `aegishv.json_write_failures_total` | Counter only. |
| `aegishv_spool_write_failures_total` metric | pipeline health metric | `aegishv.spool_write_failures_total` | Counter only. |
| `aegishv_spool_dropped_total` metric | pipeline loss metric | `aegishv.spool_dropped_total` | Counter only. |
| `aegishv_syslog_write_failures_total` metric | mirror sink health metric | `aegishv.syslog_write_failures_total` | No destination address label. |
| `aegishv_journald_write_failures_total` metric | mirror sink health metric | `aegishv.journald_write_failures_total` | No socket path label. |
| future OTLP export failures | mirror/export health metric | `aegishv.otlp_write_failures_total` | Design-only; not implemented. |

If a future OCSF/ECS exporter drops mirrored events after JSONL succeeds, report that as export loss, not source telemetry loss.

## Current Status

No OCSF or ECS runtime exporter exists in this tree. No config section enables OCSF or ECS output. This document is a mapping reference for future code or external transforms and does not change AegisHV JSONL, metrics, syslog, journald, replay, or snapshot behavior.
