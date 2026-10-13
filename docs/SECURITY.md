# Security

The sensor trusts the host kernel, tracefs, `/proc`, config files, and selected
QMP sockets. Host compromise can alter its input and output. Keep metrics local,
run under a dedicated account where possible, and grant only the paths and
sockets the deployment needs.

QMP controls a VM. Keep stable ID matching enabled, verify process start times,
and review the identity-confidence requirement before allowing actions. Dump
paths must stay under the configured root and cannot cross symlinks. Events
record action decisions, retries, timeouts, and refusal reasons.

Config validation rejects unknown or malformed values at startup. Collector
control messages use a separate channel from lossy telemetry. Queue/output loss
is recorded in events and metrics.

The systemd units restrict process privileges and filesystem access. Optional
seccomp, AppArmor, and SELinux files are shipped for local review; they are not
automatically enforced by the binary. Release checksums, SBOMs, signatures, and
provenance are described in [RELEASE.md](../RELEASE.md).

Runtime events are unsigned and unredacted. Audit files are append-oriented
local files, not tamper-proof storage. Policy bundle verification needs an
external trust root. Remote authentication, secret management, and encrypted
dump storage are deployment responsibilities.

The lab kernel is experimental and has no complete hostile-guest or DMA
isolation boundary. Live confidential-guest inspection (SEV/SEV-ES/SEV-SNP, TDX,
pKVM, Arm CCA) is unsupported. See [the threat model](THREAT_MODEL.md).
