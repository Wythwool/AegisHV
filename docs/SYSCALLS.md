# SYSCALLS

Policy file contains **path rules** and optional **callsite hash allow‑lists**.

Example allow:
- /usr/bin/ssh
- /usr/sbin/nginx
- /bin/bash (hash = 0x2a5e...)

Everything else default‑deny in restricted sandboxes.
