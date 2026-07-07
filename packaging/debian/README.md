# Debian Packaging

The Debian package installs the current AegisHV host-side sensor and operator
files. It does not enable or start the service during installation.

Installed layout:

- `/usr/bin/aegishv`
- `/etc/aegishv/config.toml`
- `/usr/lib/systemd/system/aegishv.service`
- `/usr/lib/tmpfiles.d/aegishv.conf`
- `/usr/share/aegishv/schema/*.json`
- `/usr/share/aegishv/scripts/`
- `/usr/share/doc/aegishv/`
- `/var/lib/aegishv`, `/var/lib/aegishv/dumps`, `/var/lib/aegishv/spool`
- `/var/log/aegishv`
- `/run/aegishv`

The package creates the `aegishv` system user and group only if they do not
already exist. Runtime directories are created with mode `0750` and owned by
`aegishv:aegishv`.

Installing the package does not grant tracefs, KVM, libvirt, or QMP access.
Operators still need host-specific permissions before starting the service.
