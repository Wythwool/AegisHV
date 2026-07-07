# AppArmor Profile

`usr.bin.aegishv` is an optional AppArmor profile for the current AegisHV
host-side Linux userspace sensor.

The profile allows:

- the packaged AegisHV binary and shared libraries;
- `/etc/aegishv` config reads;
- `/usr/share/aegishv` schema and hardening artifact reads;
- tracefs reads under `/sys/kernel/tracing` and `/sys/kernel/debug/tracing`;
- bounded `/proc` reads used for process identity metadata;
- JSONL, spool, dump, and snapshot writes under `/var/log/aegishv`,
  `/var/lib/aegishv`, and `/run/aegishv`;
- QMP Unix sockets under common libvirt QEMU runtime directories;
- journald and syslog datagram sockets;
- TCP metrics listener and UDP syslog networking.

The profile is not installed into `/etc/apparmor.d` by package scripts and is
not enabled by the packaged systemd units. Operators must copy, tune, load, and
test it in their own deployment before enforcement.

This profile restricts filesystem and process access where practical. It is not
complete sandboxing, kernel isolation, exploit prevention, type-1 safety, VMI
enforcement, EPT/NPT enforcement, syscall-path integrity, or hardware PMU
support.
