# Troubleshooting

## Operator failure reference

Use this table first when the process exits, readiness degrades, or an expected action is refused. The fixes are for the current Linux host-side KVM sensor only.

| Area | Symptom | Likely cause | Fix |
| --- | --- | --- | --- |
| config parser errors | `validate-config` reports a line-numbered parser error. | The handwritten config subset rejected malformed arrays, duplicate keys, unsupported section syntax, or an invalid inline action table. | Fix the line reported by `aegishv validate-config --config config.example.toml`, then restart or send `SIGHUP` only after validation passes. |
| metrics bind failures | Startup exits after metrics listener setup. | `--listen` or the configured metrics address is already in use, malformed, or not bindable by the service user. | Use `--listen ''` to disable the listener, choose a free address, or explicitly configure degraded metrics bind behavior if losing `/metrics`, `/healthz`, and `/readyz` is acceptable for that deployment. |
| JSONL output failures | JSONL output returns a write error. | The output path parent is missing, permissions changed, the disk filled, or the target was replaced with a directory or bad handle. | Restore the path and permissions. If spool is disabled, the write failure is fatal. If spool is enabled, check spool counters and files before restarting. |
| JSONL reopen failures | `jsonl_reopen_failed` appears after `SIGHUP`. | Reopen could not create or open the configured file-backed JSONL path. | Check parent directory existence, ownership, disk space, and whether the path is a directory. The old writer is kept when reopen fails. |
| disk spool limits and failures | `aegishv_spool_dropped_total` or `aegishv_spool_write_failures_total` increases. | The optional spool hit `max_bytes`, segment creation failed, compressed records did not fit, or the spool directory became unavailable while JSONL output was degraded. | Free space, fix ownership, reduce external interference with `spool-*.seg`, or raise configured spool limits. AegisHV does not replay spool files automatically. |
| syslog output failures | `aegishv_syslog_write_failures_total` increases or startup rejects `[syslog]`. | The optional syslog sink is enabled with a bad numeric `ip:port`, unsupported facility, datagram larger than `syslog.max_message_bytes`, or a UDP send error. | Fix `[syslog]` and restart. Syslog settings are startup-only. JSONL remains the primary output; syslog is UDP only and has no acknowledgement or retry. |
| journald output failures | `aegishv_journald_write_failures_total` increases, startup rejects `[journald]`, or non-Linux startup reports unsupported journald output. | The optional journald sink is enabled on a non-Linux host, uses an empty socket path, unsafe identifier, oversized message, missing socket, or a local datagram send error. | Fix `[journald]` and restart. Journald settings are startup-only. JSONL remains the primary output; journald has no acknowledgement, retry, daemon health check, or remote transport. |
| tracefs format diagnostics | Startup or snapshot reports `tracefs_format_diagnostic`. | `events/kvm/kvm_exit/format` is missing, unreadable, malformed, or lacks fields the text parser expects. | Mount tracefs, fix permissions, check that KVM tracepoints exist, and run `snapshot --json <path>` to inspect `tracepoints`. Replay mode does not read tracefs metadata. |
| live tracefs smoke failures | `scripts/live-tracefs-smoke.sh` fails before waiting for data. | The host is not Linux, tracefs is not mounted, KVM tracepoints are absent, or the user cannot read/write tracefs controls. | Run it on a Linux KVM host with tracefs mounted and privileges for `trace_pipe`, `trace_marker`, `events/kvm/kvm_exit/enable`, and `tracing_on`. |
| live tracefs smoke failures | `scripts/live-tracefs-smoke.sh` times out. | No live `kvm_exit` was read after the script marker. | Start or exercise a KVM guest and rerun. Do not treat replay success as a live tracefs smoke pass. |
| QMP action refusal | QMP action audit shows `refused=true`. | Policy matched, but an action guard rejected the request before QMP execution. Common causes are dry-run, stable-QMP identity mismatch, low identity confidence, unverified or PID-only identity, unsupported action kind, or unsafe dump path. | Read `action.failure_class`, `action.result`, and `action.detail`. Fix the guard condition before retrying. |
| QMP action failure | QMP action audit shows timeout or retry exhaustion. | The QMP socket was unreachable, slow, denied by permissions, or returned an error. | Check the socket path, service user permissions, QEMU monitor availability, and action timeout. Use dry-run first when changing policy rules. |
| stable-QMP identity mismatch | Stable-QMP matching refuses VM-name fallback. | `identity.require_stable_qmp_match=true` and the event lacks a stable `vm_id` match for the configured QMP mapping. | Add or fix the QMP mapping `vm_id` pattern, improve identity enrichment, or disable the requirement only if VM-name fallback is acceptable for that host. |
| action identity confidence | QMP action audit detail includes `identity_safety: reason=low_confidence`, `pid_only_identity`, `unverified_identity`, `missing_identity`, `stale_identity`, or `conflicting_identity`. | The event identity did not meet `identity.min_action_confidence` or lacked start-time-verified stable metadata. | Refresh identity discovery, add current PID/TID start-time ticks to the XML snapshot, fix identity conflicts, or keep the rule in `dry_run`/`manual_approval`. Do not lower the threshold to `low`; config rejects it. |
| PID reuse guard | Identity is tagged ambiguous or QMP actions are refused after a PID/TID is reused. | Runtime `/proc` start-time ticks do not match the mocked libvirt XML task mapping, or start-time metadata needed to verify the mapping is unavailable. | Refresh the libvirt XML snapshot with the current task IDs and `pid_start_time_ticks` or `tid_start_time_ticks`. Do not map QMP actions by PID-only identity. |
| identity conflict diagnostics | A `sensor` event appears with reason `identity_conflict`. | Identity sources disagree, a cache entry went stale, stable metadata mismatched, or the resolver rejected a PID/TID mapping as unsafe. | Check the bounded reason tag such as `identity_conflict:libvirt_uuid_mismatch`, fix the XML snapshot or QMP mapping, then retry. Duplicate events for the same task and reason are cooled down. |
| VM inventory snapshot | `snapshot` shows an empty `vm_inventory`. | No config was provided, `identity.libvirt_xml_dir` is empty, or the current mockable discovery state has no domains. | Run `aegishv snapshot --config <file>` with a valid file-backed identity discovery directory. Empty inventory does not prove that no VMs exist. |
| dump path rejection | `dump_guest_memory` is refused. | The output path used parent traversal, a symlink, an existing file, a missing parent, or escaped `dump_root`. | Write to a new file under an existing safe `dump_root` directory. Do not point dumps at shared or symlinked paths. |
| health readiness degraded states | `/healthz` is 200 but `/readyz` is 503. | The process is alive but degraded: collector, output, queue, PMU, policy, or action state needs attention. | Read the JSON `components` object and the Prometheus counters. Fix the degraded component instead of restarting blindly. |
| reload behavior | `SIGHUP` does not apply a change. | The setting is startup-only, no `--config` path was provided, or the reload failed validation. | Validate the config first. Restart for metrics listener, PMU, tracefs/replay source, queue size, JSONL destination path, and spool setting changes. |
| shutdown behavior | `SIGINT` or `SIGTERM` shutdown looks delayed. | The process is draining collector, PMU fallback sleep, output flush, or worker joins. | Wait for the shutdown event when JSONL output is healthy. If it remains stuck, inspect tracefs access and output path state. |
| systemd failures | systemd restarts the service repeatedly. | The unit is restarting after fatal config, metrics bind, tracefs, output, or spool setup errors. | Run `aegishv validate-config --config <path>` as the service user, check `journalctl -u <unit>`, and verify tracefs/output permissions before enabling restart policies. |
| PMU limitations | PMU output has null counters or is absent. | The current PMU path is a host-thread fallback heartbeat, not full PEBS/IBS/SPE sampling. It also depends on config, permissions, and vCPU thread discovery. | Check `pmu.enable`, kernel `perf_event_paranoid`, service permissions, and `/proc/<pid>/task` visibility. |

## I get no events

- Make sure tracefs is mounted.
- Make sure `events/kvm/enable` is on.
- Check permissions on `trace_pipe`.
- Validate the host actually emits `kvm_exit` trace lines.
- Run replay mode first to separate parser/config issues from host tracefs issues.

## Tracefs format metadata is unhealthy

Live mode reads `events/kvm/kvm_exit/format` at startup. Snapshot mode reports the same check in `tracepoints_ok` and `tracepoints`.

If the metadata is missing, unreadable, malformed, or missing fields the parser expects, live mode emits a `sensor` event with reason `tracefs_format_diagnostic`. Replay mode does not read tracefs metadata.

This diagnostic checks tracepoint metadata only. It does not prove binary/perf ingestion, type-1 support, VMI, or EPT/NPT enforcement.

## Health or readiness is failing

`/healthz` and `/readyz` are served by the metrics listener. If `--listen ''` is used, these endpoints are disabled with the listener.

Read the JSON `components` object first. `collector=failed` means the input path hit a fatal collector error. `output=degraded` means at least one event had to use the optional spool after a JSONL write failed. `output=failed` means output and spool handling could not preserve an event. `queue=degraded` means the ingest queue reached capacity. `actions=degraded` means an action path returned a non-OK result. `pmu=disabled` is expected unless PMU fallback events are enabled.

`/healthz` can return HTTP 200 while `status=degraded`; the process is alive but needs operator attention. `/readyz` returns HTTP 503 for degraded state, startup, shutdown, or fatal pipeline errors.

## I do not see lifecycle events

Lifecycle events are written to the configured JSONL output. A startup event with reason `sensor_startup` is emitted after config, metrics listener state, collector startup, output, and optional spool setup are known. If startup fails before JSONL output opens, there is no JSONL destination for that event.

A shutdown event with reason `sensor_shutdown` is emitted during orderly shutdown after collector and PMU threads have been joined where practical. If the output path and the optional spool both fail during shutdown, the process reports the write error instead of pretending the event was persisted.

## Replay never exits

Replay EOF now goes over a dedicated control channel. If replay still hangs, check for an external wrapper holding stdout open or a supervisor waiting on its own lifecycle. The unit test `replay_eof_survives_full_telemetry_queue` covers the old queue-full failure mode.

## Shutdown signal does not exit promptly

`SIGINT` and `SIGTERM` set the runtime stop flag. The sensor emits a `sensor` event with reason `shutdown_signal`, stops collector and PMU loops, flushes JSONL, and joins worker threads where practical. PMU fallback sleep is checked in short intervals so a large `pmu.sample_ms` does not hold shutdown until the full sample period expires.

## SIGHUP reload did not change behavior

`SIGHUP` reloads the file passed with `--config` and reopens file-backed JSONL output. If the process started without `--config`, the sensor emits `config_reload_skipped` and keeps the current defaults.

A malformed reload emits `config_reload_failed` and keeps the last good config. Run `aegishv validate-config --config <path>` before sending `SIGHUP`.

Metrics listener settings, PMU settings, tracefs/replay source, queue size, JSONL destination path, event spool settings, syslog settings, and journald settings are startup-only. Restart the process to change them.

## JSONL rotation did not switch to the new file

Use file-backed output, not `--jsonl -`. Rotate the file by moving the old path out of the way, then send `SIGHUP`. AegisHV flushes the old writer and opens the same configured path again.

If `jsonl_reopen_failed` appears, the old writer is kept. Check the target directory permissions, parent existence, and whether the path is a directory.

## Event spool is filling or failing

The spool is opt-in. Check `[spool] enable`, `dir`, `max_bytes`, `segment_bytes`, and `compression` in the startup config. A bad spool directory or unsupported compression value fails startup when enabled.

If `aegishv_spool_dropped_total` or `aegishv_spool_write_failures_total` increases, the main JSONL output failed and the spool could not preserve at least one event. Check free space, ownership, and whether external tooling is removing or shipping old `spool-*.seg` files. `spool.compression = "rle"` can shrink repeated JSON bytes but can also grow records that do not compress well. AegisHV does not replay spool files automatically.

## QMP actions fail

- Verify the socket path.
- Verify the rule/action is not in dry-run mode.
- Verify the runtime user can connect to the socket.
- Verify the VM regex matches the enriched event `vm` field. Identity enrichment may rewrite `vm` from raw trace comm to a QEMU/libvirt name when available.
- If `identity.require_stable_qmp_match=true`, verify the event has `vm_id` and an `actions.qmp` `vm_id` pattern matches it. VM-name fallback is refused in this mode.
- Use `mode = "dry_run"` first to validate policy matching without touching the VM.

Action audit events use reason `policy_action`. Read `action.result`, `action.failure_class`, `action.attempt`, `action.retry_count`, `action.timed_out`, and `action.refused` before using the free-form `action.detail` text.

## VM name is wrong or unstable

The current identity layer is best-effort. It reads `/proc/<pid>/cmdline`, `/proc/<pid>/cgroup`, QEMU `-name`, `-uuid`, and common QMP socket paths. If the host does not expose useful command-line or cgroup data, the fallback is `host-pid:<pid>`.

For fleets, wire in real libvirt lifecycle discovery before treating `vm_id` as authoritative.

## PMU samples are missing

- Check `pmu.enable = true`.
- Check kernel `perf_event_paranoid` and service permissions.
- Check whether QEMU vCPU threads are discoverable under `/proc/<pid>/task`.
- If QEMU restarted under a reused PID/TID, the PMU fallback drops the stale target until rediscovery sees matching process start-time ticks.
- PMU output is still a host-thread target heartbeat with unavailable counters reported as `null`, not full PEBS/IBS/SPE sampling.
