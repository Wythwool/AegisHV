# Deployment

AegisHV is currently deployed as a Linux host-side sensor.

## Permissions

The service needs read access to tracefs and access to the configured QMP sockets if actions are enabled. Do not run a broad privileged container outside a lab.

Recommended host setup:

- create a dedicated `aegishv` user and group;
- grant tracefs read access through host policy or a narrow helper;
- place QMP sockets in a group-readable directory for the `aegishv` group;
- keep `/var/lib/aegishv/dumps` owned by root/aegishv and not group/world-writable;
- keep `/var/lib/aegishv/spool` owned by root/aegishv and not group/world-writable if the optional event spool is enabled;
- write JSONL to a directory with log rotation or an external spooler;
- enable UDP syslog only when a local or network collector is explicitly configured.
- enable journald only on Linux hosts where writing to the systemd journal datagram socket is intended.

## Debian Package Layout

The Debian package metadata lives under `packaging/debian`. A local package build uses:

```bash
cargo build --locked --release
bash ./scripts/package-debian.sh x86_64-unknown-linux-gnu
```

The package installs:

- `/usr/bin/aegishv`;
- `/etc/aegishv/config.toml`;
- `/usr/lib/systemd/system/aegishv.service`;
- `/usr/lib/tmpfiles.d/aegishv.conf`;
- `/usr/share/aegishv/schema/event.schema.json` and `snapshot.schema.json`;
- operator scripts under `/usr/share/aegishv/scripts`;
- docs under `/usr/share/doc/aegishv`.

The maintainer script creates the `aegishv` system group and user only if they do not already exist. It creates `/var/lib/aegishv`, `/var/lib/aegishv/dumps`, `/var/lib/aegishv/spool`, `/var/log/aegishv`, and `/run/aegishv` as `aegishv:aegishv` with mode `0750`.

The package does not enable or start the service automatically. Operators must review `/etc/aegishv/config.toml`, grant tracefs/QMP permissions for the target host, then explicitly run `systemctl enable --now aegishv` if that is intended.

Debian packaging does not add type-1 support, full VMI, EPT/NPT enforcement, syscall-path integrity, hardware PMU sampling, live libvirt integration, or package signing by itself.

## RPM Package Layout

The RPM package metadata lives under `packaging/rpm`. A local package build uses:

```bash
cargo build --locked --release
bash ./scripts/package-rpm.sh x86_64-unknown-linux-gnu
```

The package installs:

- `/usr/bin/aegishv`;
- `/etc/aegishv/config.toml`;
- `/usr/lib/systemd/system/aegishv.service`;
- `/usr/lib/tmpfiles.d/aegishv.conf`;
- `/usr/share/aegishv/schema/event.schema.json` and `snapshot.schema.json`;
- operator scripts under `/usr/share/aegishv/scripts`;
- docs under `/usr/share/doc/aegishv`.

The RPM scriptlets create the `aegishv` system group and user only if they do not already exist. They create `/var/lib/aegishv`, `/var/lib/aegishv/dumps`, `/var/lib/aegishv/spool`, `/var/log/aegishv`, and `/run/aegishv` as `aegishv:aegishv` with mode `0750`.

The RPM package does not enable or start the service automatically. Operators must review `/etc/aegishv/config.toml`, grant tracefs/QMP permissions for the target host, then explicitly run `systemctl enable --now aegishv` if that is intended.

RPM packaging does not add type-1 support, full VMI, EPT/NPT enforcement, syscall-path integrity, hardware PMU sampling, live libvirt integration, package signing, or guaranteed delivery.

## Container Image Labels and Limits

The repository has a Dockerfile and CI runs a Docker build smoke. There is no current release workflow that publishes or signs container images.

The Dockerfile sets bounded OCI image labels in the final image:

- `org.opencontainers.image.title`;
- `org.opencontainers.image.description`;
- `org.opencontainers.image.source`;
- `org.opencontainers.image.url`;
- `org.opencontainers.image.documentation`;
- `org.opencontainers.image.version`;
- `org.opencontainers.image.revision`;
- `org.opencontainers.image.created`;
- `org.opencontainers.image.licenses`;
- `org.opencontainers.image.authors`;
- `org.opencontainers.image.vendor`.

Release builds should pass concrete build arguments for `AEGISHV_VERSION`, `AEGISHV_REVISION`, and `AEGISHV_CREATED`:

```bash
docker build \
  --build-arg AEGISHV_VERSION=0.4.0 \
  --build-arg AEGISHV_REVISION=$(git rev-parse HEAD) \
  --build-arg AEGISHV_CREATED=$(date -u +%Y-%m-%dT%H:%M:%SZ) \
  -t aegishv:0.4.0 .
```

Verify labels locally with:

```bash
docker image inspect aegishv:0.4.0 --format '{{ index .Config.Labels "org.opencontainers.image.source" }}'
docker image inspect aegishv:0.4.0 --format '{{ index .Config.Labels "org.opencontainers.image.revision" }}'
```

The `.dockerignore` excludes build outputs, repository metadata, cache directories, `dist/`, package outputs, and old archives from the Docker build context.

Running AegisHV in a container still needs host-specific mounts and permissions. At minimum, operators must provide a reviewed config, a writable JSONL/log destination, and tracefs access for live collection. QMP actions also need explicit socket mounts and group permissions. Do not run a broad privileged container outside a lab.

Container signing is documentation-only in this tree. If a release workflow later publishes an image, it should sign the image digest with real Sigstore/Cosign keyless signing and verify it with the workflow OIDC identity. A future verification command should use the published image digest, the repository workflow identity, and the token issuer:

```bash
cosign verify ghcr.io/nullbit1/aegishv:0.4.0 \
  --certificate-identity https://github.com/Nullbit1/AegisHV/.github/workflows/release.yml@refs/tags/v0.4.0 \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

That command shape is guidance for a later publishing workflow. It is not evidence that a current AegisHV container image exists or is signed.

Container signing would bind an image digest to a CI identity. It would not prove runtime confinement, safe host mounts, vulnerability status, reproducible builds, source review quality, type-1 support, full VMI, EPT/NPT enforcement, syscall-path integrity, live libvirt integration, hardware PMU sampling, or that the registry and CI provider were uncompromised.

## Optional Seccomp Profile

The optional seccomp profile lives at `packaging/seccomp/aegishv-seccomp.json`. Debian and RPM packages ship it as `/usr/share/aegishv/seccomp/aegishv-seccomp.json`, but the packaged systemd units do not enable it. Operators must test the profile with their own config, mounts, QMP socket layout, output sinks, and distro runtime before enforcing it.

The profile is an OCI-style JSON profile with `defaultAction = SCMP_ACT_ERRNO`. It permits syscall groups for:

- process startup, shutdown, signals, memory mapping, threads, and Rust runtime support;
- config, tracefs, JSONL, spool, snapshot, schema, and procfs file access;
- timers, polling, and readiness loops;
- metrics TCP listener, QMP Unix sockets, UDP syslog, and journald datagram writes.

Everything not listed is denied by default. Expected blocked examples include `bpf`, `ptrace`, `perf_event_open`, `mount`, `umount2`, `init_module`, `delete_module`, `kexec_load`, `reboot`, and keyring syscalls. The profile still allows `clone`/`clone3` for normal thread creation and does not inspect every syscall argument.

Docker example for a lab run:

```bash
docker run --rm \
  --security-opt seccomp=packaging/seccomp/aegishv-seccomp.json \
  -v /sys/kernel/tracing:/sys/kernel/tracing:ro \
  -v /var/log/aegishv:/var/log/aegishv \
  aegishv:0.4.0
```

Podman uses the same profile shape:

```bash
podman run --rm \
  --security-opt seccomp=packaging/seccomp/aegishv-seccomp.json \
  -v /sys/kernel/tracing:/sys/kernel/tracing:ro \
  -v /var/log/aegishv:/var/log/aegishv \
  aegishv:0.4.0
```

systemd does not consume OCI seccomp JSON directly. Use a service override only after testing the deployment with the same config and sinks:

```ini
[Service]
SystemCallErrorNumber=EPERM
SystemCallFilter=@system-service @file-system @network-io @signal
```

That systemd snippet is not installed by the package and is not equivalent to the OCI JSON profile. Treat it as a starting point for local testing, not as a default policy.

The seccomp profile can break deployments that add DNS/NSS behavior, TLS exporters, live libvirt daemon clients, hardware PMU/perf usage, extra helper processes, distro-specific libc startup behavior, or output sinks that need additional syscalls. A denied syscall normally returns `ENOSYS` from the OCI profile or `EPERM` from the systemd example.

The profile reduces syscall surface where practical. It does not prove complete sandboxing, kernel isolation, exploit prevention, type-1 safety, full VMI, EPT/NPT enforcement, syscall-path integrity, live libvirt integration, hardware PMU support, or safe host mounts.

## Optional AppArmor Profile

The optional AppArmor profile lives at `packaging/apparmor/usr.bin.aegishv`. Debian and RPM packages ship it as `/usr/share/aegishv/apparmor/usr.bin.aegishv`, but they do not install it into `/etc/apparmor.d` and the packaged systemd units do not enable it. Operators must test the profile with their own config, tracefs permissions, QMP socket layout, output sinks, spool/dump paths, and distro AppArmor policy before enforcing it.

The profile permits:

- `/usr/bin/aegishv` and `/usr/local/bin/aegishv` execution;
- dynamic linker and shared library reads;
- `/etc/aegishv` config reads;
- `/usr/share/aegishv` schemas, hardening profiles, scripts, and docs reads;
- tracefs reads under `/sys/kernel/tracing` and `/sys/kernel/debug/tracing`;
- bounded `/proc` reads for process identity metadata;
- JSONL, spool, dump, and snapshot writes under `/var/log/aegishv`, `/var/lib/aegishv`, and `/run/aegishv`;
- QMP Unix socket access under common libvirt QEMU runtime directories;
- journald and syslog datagram socket writes;
- TCP metrics listener, UDP syslog networking, and Unix stream/datagram sockets.

Everything else is denied by AppArmor unless another local policy grants it. The profile explicitly denies `/root` access, home-directory writes, `/tmp` execution, and `/etc/shadow` reads. It does not grant broad writes outside the AegisHV log, state, and runtime directories.

To test it on a Debian or RPM host, copy the shipped profile into the local AppArmor policy directory, load it, and use complain mode first:

```bash
sudo install -m 0644 /usr/share/aegishv/apparmor/usr.bin.aegishv /etc/apparmor.d/usr.bin.aegishv
sudo apparmor_parser -r /etc/apparmor.d/usr.bin.aegishv
sudo aa-complain aegishv
sudo systemctl restart aegishv
```

After testing replay, live tracefs, metrics, QMP dry runs, syslog/journald output, spool, dumps, and snapshots under the deployment's actual paths, enforce it with:

```bash
sudo aa-enforce aegishv
sudo systemctl restart aegishv
```

If the service manager supports explicit profile selection, use a local override only after loading and testing the profile:

```ini
[Service]
AppArmorProfile=aegishv
```

That override is not installed by the package. It may not be available on every distro or systemd build.

The profile can break deployments that use non-default config paths, non-libvirt QMP socket directories, custom spool/dump/log paths, custom snapshot paths, DNS/NSS behavior not covered by the base file rules, TLS exporters, live libvirt daemon clients, helper processes, package-specific library paths, or container runtime paths. Adjust the profile locally before enforcing it; it must be adjusted per deployment when paths or sinks differ.

The profile restricts filesystem and process access where practical. It does not prove complete sandboxing, kernel isolation, exploit prevention, type-1 safety, full VMI, EPT/NPT enforcement, syscall-path integrity, live libvirt integration, hardware PMU support, or safe host mounts.

## Optional SELinux Policy Skeleton

The optional SELinux policy skeleton lives under `packaging/selinux`. Debian and RPM packages ship it as `/usr/share/aegishv/selinux`, but they do not load it with `semodule`, relabel files, put the domain into enforcing mode, or change packaged systemd defaults. Operators must build, review, tune, load, and test the skeleton on the target distribution before enforcing it.

The skeleton provides:

- an `aegishv_t` process domain and `aegishv_exec_t` binary type;
- file contexts for `/usr/bin/aegishv`, `/usr/local/bin/aegishv`, `/etc/aegishv`, `/usr/share/aegishv`, `/usr/share/doc/aegishv`, `/var/log/aegishv`, `/var/lib/aegishv`, and `/run/aegishv`;
- config, schema, hardening profile, and documentation reads;
- tracefs/debugfs, sysfs, configfs, and procfs reads used by the current sensor, using common SELinux labels such as `tracefs_t` and `debugfs_t`;
- JSONL, spool, dump, state, and snapshot writes under AegisHV-owned directories;
- QMP Unix socket access for common libvirt QEMU runtime labels;
- syslog and journald datagram socket writes;
- TCP listener and UDP sender rules for metrics and syslog.

Example review flow on a host with SELinux policy development tooling:

```bash
cd /usr/share/aegishv/selinux
make -f /usr/share/selinux/devel/Makefile aegishv.pp
sudo semodule -i aegishv.pp
sudo restorecon -Rv /usr/bin/aegishv /etc/aegishv /usr/share/aegishv /var/log/aegishv /var/lib/aegishv /run/aegishv
sudo semanage permissive -a aegishv_t
sudo systemctl restart aegishv
```

Run replay, live tracefs smoke, metrics listener checks, configured QMP action dry runs, syslog/journald output checks, spool checks, dump-path checks, and snapshot checks while `aegishv_t` is permissive. Review audit denials and adjust local file contexts or allow rules before enforcing it:

```bash
sudo semanage permissive -d aegishv_t
sudo systemctl restart aegishv
```

The skeleton can break deployments that use non-default config paths, non-libvirt QMP socket labels, custom spool/dump/log paths, custom snapshot paths, distro-specific tracefs labels, live libvirt daemon clients, TLS exporters, helper processes, or output sinks with additional SELinux labels. Adjust the policy locally; it must be adjusted per deployment when paths, labels, or sinks differ.

Tracefs labeling is distro-specific. The skeleton includes read rules for common `tracefs_t` and `debugfs_t` labels, but operators may still need local file contexts or allow rules when `/sys/kernel/tracing` or `/sys/kernel/debug/tracing` uses a different label.

The skeleton is a starting policy, not complete confinement. It does not prove kernel isolation, exploit prevention, type-1 safety, full VMI, EPT/NPT enforcement, syscall-path integrity, live libvirt integration, hardware PMU support, or safe host labeling.

## Dump safety

`dump_guest_memory` validates the requested output path before sending the QMP command. The output path must be absolute, must not contain `..`, must not already exist, must not be a symlink, must have an existing directory parent, and the parent must stay under `actions.dump_root` after canonicalization. AegisHV also rejects symlink ancestors in the output parent path, rejects a missing or non-directory `actions.dump_root`, rejects a symlink `actions.dump_root`, rejects symlink ancestors in `actions.dump_root`, and rejects group/world-writable dump roots or output parents on Unix.

These checks reduce unsafe path mistakes. They do not make QEMU's write atomic with AegisHV's validation. QEMU opens the file after AegisHV validates the path, so a hostile writable dump directory can still create a time-of-check/time-of-use race. Treat the dump root and every writable parent below it as part of the trusted computing base. Do not place dump roots in tenant-writable, guest-writable, shared scratch, or externally rotated directories.

## Action audit events

Each policy action emits a `policy` JSONL event with reason `policy_action`. The nested `action` object records the final decision and result, attempt count, maximum attempts, retry count, configured timeout, timeout flag, refusal flag, and a bounded failure class.

`failure_class` is one of `qmp_error`, `timeout`, `stable_identity_required`, `unsupported_action`, `unsafe_input`, `missing_argument`, or `null`. Do not treat `detail` as a metric label; it is operator text and may contain existing error detail.

QMP-backed actions (`pause_vm`, `resume_vm`, `dump_guest_memory`, and `quarantine_nic`) require a non-ambiguous identity with confidence at least `identity.min_action_confidence`, plus PID/TID start-time verification. The default threshold is `high`; `medium` is accepted as a configured threshold but still requires start-time verification. `low` is rejected at config load. PID-only fallback identity, missing identity metadata, stale identity cache conflicts, and conflicting identity sources refuse before QMP socket selection. `dry_run`, `noop`, and `manual_approval` do not execute QMP and are allowed without raising identity confidence.

## Optional libvirt XML identity discovery

`identity.libvirt_xml_dir` is disabled by default. When set, it points at a directory of domain XML files that AegisHV reads at startup or reload. This is file-backed discovery for tests or operator-maintained snapshots. It does not connect to a libvirt daemon or prove VM inventory freshness.

Each XML file must include normal domain `name` and `uuid` elements. AegisHV also expects mocked AegisHV metadata attributes for host task mapping, such as `pid`, `tid`, and `qmp_socket`. `pid_start_time_ticks`, `tid_start_time_ticks`, or tag-local `start_time_ticks` can bind a PID/TID mapping to the Linux `/proc/<pid>/stat` start time. When runtime start-time data is available and the XML mapping lacks or contradicts it, AegisHV treats that host task as unsafe for stable identity and QMP action selection. A matched PID or TID enriches events with `vm_id = libvirt:<uuid>`, `vm_name`, and a QMP socket hint.

Configure `actions.qmp` with `vm_id = "libvirt:<uuid>"` when a libvirt UUID is available. UUID-authoritative mappings are resolved before VM-name mappings. If more than one matching UUID mapping points at different sockets, AegisHV refuses the QMP action instead of falling back to the VM name. VM-name fallback is used only when `identity.require_stable_qmp_match = false` and no stable mapping was selected. Fallback also refuses an entry whose `vm_id` pattern conflicts with the event's stable ID.

Thread metadata may include `vcpu_id` on a QEMU thread element, for example `tid="4243" vcpu_id="0"`. When a trace event carries that host TID and the tracepoint did not already expose a guest vCPU, AegisHV fills `vcpu_id` and the backward-compatible `vcpu` alias from the metadata. `host_cpu` remains the Linux trace header CPU and is not treated as a guest vCPU. Missing or conflicting vCPU metadata leaves `vcpu_id` unset; AegisHV does not invent guest vCPU IDs.

If more than one XML domain maps the same host task, identity is marked ambiguous. QMP actions are refused before socket selection for ambiguous identity, even when VM-name fallback is otherwise allowed. Fix the XML snapshot or identity source before retrying actions.

Identity metadata is emitted in each enriched event under `identity`. Sources are fixed strings: `trace_comm`, `proc_cmdline`, `proc_cgroup`, `libvirt_xml`, `libvirt_lifecycle`, `qmp_socket_hint`, `fallback_pid`, `ambiguous`, and `start_time_verified`. Confidence is bounded to `low`, `medium`, or `high`. Trace comm and PID-only fallback are low confidence. Libvirt UUID metadata without start-time verification is medium. High confidence requires libvirt UUID metadata and a matching observed PID/TID start-time tick. Missing data never upgrades confidence.

Identity conflicts emit a bounded `sensor` event with reason `identity_conflict`. Reasons are fixed strings: `multiple_domains`, `pid_reuse`, `start_time_unverified`, `stale_cache`, `proc_cgroup_mismatch`, `libvirt_uuid_mismatch`, `qmp_socket_mismatch`, and `libvirt_name_mismatch`. The event reports only the bounded reason, task id, identity metadata, and outcome. It does not include raw XML, command lines, socket paths, cgroup paths, or VM-name-derived labels. Repeated events for the same task and reason are suppressed for a short cooldown.

The resolver has a small mockable lifecycle update interface. A supplied start, stop, pause, resume, or migrate update can refresh or invalidate the in-memory identity cache and emits a `sensor` event with reason `libvirt_lifecycle`. The current binary still does not connect to a live libvirt daemon or subscribe to libvirt lifecycle events. Use the XML snapshot path as the operator-facing discovery source until a real libvirt event loop exists and is tested.

`aegishv snapshot --config <file>` includes `vm_inventory` from the configured identity discovery state. The inventory reports UUID/name, known host task ids, vCPU mappings, QMP socket presence, source/confidence, and bounded ambiguity/conflict state. It does not include raw XML, raw command lines, socket paths, host paths, or arbitrary discovery errors. Without `identity.libvirt_xml_dir` or lifecycle metadata, the inventory is empty and does not prove that no VMs exist.

## systemd

Use `packaging/systemd/aegishv.service` as the starting point. Real deployments need distro-specific tracefs/QMP permissions and may need `SupplementaryGroups=` for the QMP socket group.

## Metrics listener startup

An empty `--listen ''` disables the metrics listener. Any non-empty listener address must bind successfully by default. If the port is already in use or the address is invalid, startup exits instead of running without `/metrics`, `/healthz`, and `/readyz`.

Set `metrics.allow_bind_failure = true` only for an explicit degraded mode where losing the local metrics listener is acceptable. In that mode the sensor logs the bind failure, records the metrics listener as degraded, and continues without a metrics thread.

## Health and readiness

`/healthz` and `/readyz` exist only when the metrics listener is enabled. `--listen ''` still disables the listener and these endpoints.

`/healthz` is a liveness check. It returns HTTP 200 while the process is starting, running, or degraded, and HTTP 503 after a fatal runtime component failure or during shutdown. `status` in the JSON body still reports `starting`, `ok`, `degraded`, `failed`, `stopping`, or `stopped`.

`/readyz` is stricter. It returns HTTP 200 only when the runtime is running, the collector is running, output is writable, policy and action handling are not degraded, the ingest queue is not full, and enabled PMU and metrics listener state is acceptable. It returns HTTP 503 for startup, shutdown, degraded output/spool state, queue pressure, collector failure, action failures, and fatal pipeline errors.

The response includes component states for `runtime`, `collector`, `metrics_listener`, `output`, `policy`, `pmu`, `queue`, and `actions`. These are process-local checks from the current userspace sensor. They do not prove guest integrity, hypervisor integrity, type-1 operation, full VMI, or PMU sampling quality.

## Trace input metrics

`aegishv_trace_inputs_total` uses a bounded `reason` label: `parsed`, `unrelated_tracepoint`, `unsupported_line`, `malformed_kvm_exit`, `parser_degraded`, or `parser_bug`. These labels are fixed in code and must not be built from raw trace lines, VM names, file paths, or error strings.

## Identity metrics

Identity metrics use fixed label sets. `aegishv_identity_cache_lookups_total` reports `result` as `hit`, `miss`, or `refusal`. `aegishv_identity_enrichments_total` reports `confidence` as `low`, `medium`, or `high` with an `ambiguous` boolean. `aegishv_identity_conflicts_total` reports the same bounded conflict reasons used by `identity_conflict` events. `aegishv_identity_qmp_safety_refusals_total` reports only fixed reasons: `stable_identity_required`, `ambiguous_identity`, `conflicting_stable_mapping`, `missing_identity`, `low_confidence`, `unverified_identity`, `pid_only_identity`, `stale_identity`, or `conflicting_identity`.

`aegishv_identity_inventory_vms` and `aegishv_identity_inventory_degraded` are process-local gauges from the current identity discovery state. They do not prove live libvirt freshness. Do not add VM names, UUIDs, PIDs, socket paths, raw XML, command lines, or free-form errors as metric labels.

## Lifecycle events

AegisHV emits `sensor` lifecycle events to JSONL when the run starts and when it shuts down.

Startup uses reason `sensor_startup`. The message reports the AegisHV version, run mode (`tracefs` or `replay`), config source (`defaults` or `file`), JSONL target class (`stdout` or `file`), metrics listener state, queue capacity, policy rule and QMP mapping counts, identity setting, stable QMP requirement, PMU fallback state, spool state, and W^X timing settings. It intentionally does not include configured paths, QMP socket names, or claims about unsupported backends.

Shutdown uses reason `sensor_shutdown`. The message reports whether shutdown was clean, signal-driven, or caused by a known fatal pipeline error, plus dropped-event and output/spool counters. Signal shutdown still emits the existing `shutdown_signal` event before the final lifecycle event.

## Event spool

`spool.enable = false` preserves the existing behavior: a JSONL write failure is fatal. When `spool.enable = true`, AegisHV creates `spool.dir`, appends events that fail to write to the main JSONL output, and counts preserved, failed, and dropped spool attempts in metrics.

Spool segment files are append-only `spool-*.seg` files. With `spool.compression = "none"`, each segment starts with `aegishv-spool-v1 len-hex-jsonl`; each record is a 16-byte hexadecimal JSON length, one space, the JSON event, and a newline. AegisHV flushes and syncs each spooled record after writing it. The length prefix lets offline tooling reject a torn final record after a crash.

`spool.compression = "rle"` is opt-in. It writes `aegishv-spool-v2 compression=rle record=hex-u64-uncompressed-hex-u64-payload` segments with a small internal run-length codec. This is real compression for repeated bytes, requires no runtime shell commands, and may grow records that do not compress well. `spool.max_bytes` still counts on-disk bytes after the segment header and encoded record payload. Corrupt or unsupported segment headers should be rejected by offline tooling instead of guessed.

The spool is bounded by `spool.max_bytes`. `spool.segment_bytes` controls when a new segment is opened. If the spool is full or cannot be written, the event is counted in `aegishv_spool_dropped_total`, the write failure is counted in `aegishv_spool_write_failures_total`, and the process returns an explicit error instead of discarding the event silently.

Durability limits are narrow. The spool is an emergency copy for events whose direct JSONL write fails; it is not an acknowledgement protocol, it does not replay segments automatically, it does not rotate stdout, and it does not prove guaranteed delivery. Events already accepted by the buffered main writer before a later flush failure may not have a spool copy. Size the directory and ship or inspect segment files with external tooling.

## Syslog output

`syslog.enable = false` is the default and preserves existing JSONL/stdout/file/spool behavior. When enabled, AegisHV sends each emitted event as one UDP syslog datagram after JSONL write or spool handling. The payload is the same JSON event line prefixed with a small RFC5424-style header. `syslog.address` must be a numeric `ip:port`; `syslog.facility` is bounded to `user`, `daemon`, or `local0` through `local7`.

Severity mapping is fixed: `critical` maps to syslog severity 2, `high` to 3, `medium` to 4, `low` to 5, and `info` to 6. `syslog.max_message_bytes` bounds each datagram. If a configured syslog send or size check fails, AegisHV increments `aegishv_syslog_write_failures_total`, marks output failed, and returns an explicit runtime error. It does not put the destination address, VM names, UUIDs, paths, raw command lines, XML, or free-form error text into metric labels.

The syslog sink is not a delivery guarantee. It is UDP only, has no TLS, TCP, acknowledgement, retry, queue, or daemon health check, and does not replace JSONL or the optional spool. It does not implement OCSF, ECS, or SIEM-specific normalization.

## Journald output

`journald.enable = false` is the default and preserves existing JSONL/stdout/file/spool/syslog behavior. When enabled on Linux, AegisHV sends each emitted event to the configured systemd-journald Unix datagram socket after JSONL write or spool handling. The message fields are bounded to `PRIORITY`, `SYSLOG_IDENTIFIER`, `AEGISHV_CATEGORY`, `AEGISHV_SEVERITY`, and `MESSAGE`. `MESSAGE` is the same JSON event line emitted to JSONL.

Severity mapping uses the same bounded values as syslog: `critical` maps to 2, `high` to 3, `medium` to 4, `low` to 5, and `info` to 6. `journald.identifier` accepts only 1..=64 ASCII letters, digits, `.`, `_`, or `-`. `journald.max_message_bytes` bounds each datagram. If opening or writing the journald sink fails, AegisHV increments `aegishv_journald_write_failures_total` when the runtime is active, marks output failed, and returns an explicit error. It does not put socket paths, VM names, UUIDs, PIDs, command lines, XML, host paths, or free-form error text into metric labels.

The journald sink is not a delivery guarantee. It uses the local datagram socket, has no acknowledgement, retry, queue, daemon health check, remote transport, or journal cursor export, and does not replace JSONL or the optional spool. Enabling it on non-Linux hosts fails startup with an explicit unsupported-host error. It does not implement OCSF, ECS, or SIEM-specific normalization.

## Config parser subset

The config loader uses a small handwritten TOML subset. It accepts the section and array-section shapes used by `config.example.toml`, quoted string arrays, and inline match/action tables.

Malformed section headers, malformed arrays, malformed inline action tables, duplicate keys, duplicate scalar sections, and unsupported sections fail startup with a line-numbered error. Keep config changes inside the supported subset or validate them with:

```bash
aegishv validate-config --config ./config.example.toml
```

## Runtime config reload and JSONL reopen

On Unix hosts, `SIGHUP` reloads the file passed with `--config` and reopens file-backed JSONL output. A bad config reload emits a `sensor` event with reason `config_reload_failed` and keeps the last good runtime state.

If `--jsonl` is a file path, `SIGHUP` flushes the current writer and reopens the same path with create+append. This supports external log rotation that renames the old file and then signals AegisHV. If reopen fails, the sensor emits `jsonl_reopen_failed` and keeps the existing output writer. If `--jsonl -` is used, `SIGHUP` emits `jsonl_reopen_skipped`; stdout is never reopened or rotated by AegisHV.

Reloaded fields:

- policy rules and `allow.ignore_vm`;
- QMP action mappings, action timeout/retry settings, `identity.require_stable_qmp_match`, and `identity.min_action_confidence`;
- identity lookup enable/cache/socket directory settings;
- W^X detector window, cooldown, max pages, page size, and allowlist;
- `general.quiet` and `general.flush_every`, unless overridden by `--quiet` or `--no-quiet`.

Startup-only fields:

- tracefs or replay source;
- ingest queue size;
- JSONL destination path;
- event spool enablement, directory, size limits, and compression setting;
- syslog enablement, address, facility, and message size limit;
- journald enablement, socket, identifier, and message size limit;
- metrics listener address and bind-failure policy;
- PMU enablement, target, sample interval, and rediscovery interval.

W^X detector settings reload with a fresh detector state. Restart the process when changing startup-only fields or when preserving the current W^X correlation cache matters.
