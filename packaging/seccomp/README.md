# Seccomp Profile

`aegishv-seccomp.json` is an optional OCI-style seccomp profile for the current
host-side Linux userspace sensor.

The profile defaults to `SCMP_ACT_ERRNO` and allows syscall groups for:

- process startup, shutdown, signals, threads, and memory mapping;
- config, tracefs, JSONL, spool, snapshot, schema, and procfs file access;
- timers and readiness loops;
- metrics TCP listener, QMP Unix sockets, UDP syslog, and journald datagram
  writes.

The profile is not enabled by the packaged systemd units, Debian package, RPM
package, or Dockerfile. Operators must test it with their own config, mounts,
socket layout, and output sinks before enforcing it.

This profile reduces syscall surface where practical. It is not complete
sandboxing, kernel isolation, exploit prevention, type-1 safety, VMI
enforcement, EPT/NPT enforcement, syscall-path integrity, or hardware PMU
support.
