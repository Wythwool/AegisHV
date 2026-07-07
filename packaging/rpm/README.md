# RPM Packaging

The RPM packaging files install the current AegisHV host-side sensor and
operator files for RPM-based distributions.

The package layout is:

- `/usr/bin/aegishv`
- `/etc/aegishv/config.toml`
- `/usr/lib/systemd/system/aegishv.service`
- `/usr/lib/tmpfiles.d/aegishv.conf`
- `/usr/share/aegishv/schema`
- `/usr/share/aegishv/scripts`
- `/usr/share/doc/aegishv`

The package creates the `aegishv` system group and user only if they do not
already exist. It creates `/var/lib/aegishv`, `/var/lib/aegishv/dumps`,
`/var/lib/aegishv/spool`, `/var/log/aegishv`, and `/run/aegishv` with mode
`0750`.

The package does not enable or start the service automatically. Operators must
review `/etc/aegishv/config.toml`, grant tracefs and QMP socket permissions for
the target host, then explicitly enable and start the service if intended.

RPM packaging does not add type-1 support, full VMI, EPT/NPT enforcement,
syscall-path integrity, hardware PMU sampling, live libvirt integration, or
package signing by itself.
