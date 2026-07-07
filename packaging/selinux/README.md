# SELinux Policy Skeleton

The files in this directory are an optional SELinux policy skeleton for the
current AegisHV host-side Linux userspace sensor.

The skeleton defines:

- an `aegishv_t` process domain and `aegishv_exec_t` binary type;
- file contexts for `/usr/bin/aegishv`, `/usr/local/bin/aegishv`,
  `/etc/aegishv`, `/usr/share/aegishv`, `/usr/share/doc/aegishv`,
  `/var/log/aegishv`, `/var/lib/aegishv`, and `/run/aegishv`;
- config, schema, hardening profile, and documentation reads;
- tracefs/debugfs, sysfs, configfs, and procfs reads used by the current
  sensor, using the common `tracefs_t` and `debugfs_t` labels;
- JSONL, spool, dump, state, and snapshot writes under AegisHV-owned
  directories;
- QMP Unix socket access for common libvirt QEMU runtime labels;
- syslog and journald datagram socket writes;
- TCP listener and UDP sender rules for metrics and syslog.

The policy is not installed with `semodule`, not loaded by package scripts, and
not enabled by packaged systemd units. Operators must build, inspect, tune, load,
and test it on their target distribution before enforcing it.

Example review flow on a host with SELinux policy tooling:

```bash
cd /usr/share/aegishv/selinux
make -f /usr/share/selinux/devel/Makefile aegishv.pp
sudo semodule -i aegishv.pp
sudo restorecon -Rv /usr/bin/aegishv /etc/aegishv /usr/share/aegishv /var/log/aegishv /var/lib/aegishv /run/aegishv
sudo semanage permissive -a aegishv_t
sudo systemctl restart aegishv
```

Run replay, live tracefs smoke, metrics listener checks, QMP dry runs,
syslog/journald output checks, spool checks, dump-path checks, and snapshot
checks under permissive mode. Review audit denials, adjust local file contexts
or allow rules, then remove permissive mode only after the deployment-specific
policy is understood.

Tracefs labels vary by distribution and kernel policy. Some hosts label
`/sys/kernel/tracing` as `tracefs_t`; older or different policies may expose
trace data through debugfs labels such as `debugfs_t` under
`/sys/kernel/debug/tracing`. If audit logs show different labels, add local
file contexts or rules instead of assuming this skeleton is complete.

This skeleton is not complete confinement, kernel isolation, exploit prevention,
type-1 safety, VMI enforcement, EPT/NPT enforcement, syscall-path integrity, live
libvirt integration, or hardware PMU support.
