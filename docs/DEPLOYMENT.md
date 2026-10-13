# Deployment

The sensor needs read access to tracefs. Enabling KVM tracepoints also requires
write access to their control files. `/proc` visibility affects identity lookup;
QMP actions need access to the intended VM's monitor socket.

Start with replay and `aegishv validate-config --config config.example.toml`.
The parser accepts the TOML subset used by that file: sections, array sections,
quoted string arrays, and inline action/match tables. Unknown sections,
duplicate keys, and malformed values are errors.

`identity.libvirt_xml_dir` reads externally maintained domain XML snapshots.
Each file needs `<name>`, `<uuid>`, and host `pid` or `tid` attributes. These task
IDs and optional `start_time_ticks`, `vcpu_id`, and `qmp_socket` attributes are
additional AegisHV metadata. A plain domain definition without host task IDs
is rejected. The sensor does not populate this directory or subscribe to
libvirt events. PID/TID start times are still checked against `/proc` before
an identity can authorize an action.

## Installation

```sh
cargo build --locked --release
bash scripts/package-debian.sh
# or: bash scripts/package-rpm.sh
```

See [Debian](../packaging/debian/README.md) and [RPM](../packaging/rpm/README.md)
for package layout and prerequisites. Service units are in `packaging/systemd`
and `systemd`. Configure paths and permissions for the host before starting one.

The Dockerfile builds the sensor, but live use still needs host tracefs, `/proc`,
and any QMP sockets. Replay does not need privileged host access.
The optional [seccomp](../packaging/seccomp/README.md),
[AppArmor](../packaging/apparmor/README.md), and
[SELinux](../packaging/selinux/README.md) files need host-specific review.

## Actions

Use stable `actions.qmp.vm_id` mappings. By default, actions also require high
identity confidence and verified PID/TID start times. Setting
`identity.require_stable_qmp_match=false` permits the configured VM-name fallback.
An ambiguous identity still cannot choose a target.

Dump destinations must be new files below `actions.dump_root`, with no symlink
components. Keep that directory out of group/world-writable locations. A QMP
dump acceptance means the request was accepted, not that a complete dump exists.

## Output

JSONL is the primary stream. See [output notes](EVENT_EXPORT.md) for optional
syslog, journald, and spool settings. Guest addresses, VM IDs, and action details
can appear in events; [redaction is external](EVENT_REDACTION.md).

The metrics listener serves `/metrics`, `/healthz`, and `/readyz`.
Health and readiness differ: a running collector with degraded output can be
alive without being ready. A configured listener that cannot bind stops startup
unless `metrics.allow_bind_failure=true`. Use `--listen ''` to disable it.

## Reload and rotation

On Unix, `SIGHUP` reloads the `--config` file and reopens file-backed JSONL in
append mode. A bad reload retains the last good settings and emits
`config_reload_failed`. Failed reopen retains the current writer and emits
`jsonl_reopen_failed`. Stdout produces `jsonl_reopen_skipped`.

Reload covers policies, ignored VMs, action mappings/timeouts/retries, identity
settings, W^X settings/allowlist, and output quiet/flush settings. CLI quiet flags
take precedence. Reloading W^X settings clears correlation state.

Restart for source paths, queue size, JSONL destination, spool, syslog, journald,
metrics-listener settings, or PMU settings. For log rotation, rename the old
JSONL file and then send `SIGHUP`.
