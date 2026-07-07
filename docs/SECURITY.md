# Security model

## Runtime posture

- Run as a dedicated service account.
- Bind metrics to localhost by default.
- Treat QMP sockets as privileged control channels.
- Keep `identity.require_stable_qmp_match=true` unless a migration explicitly accepts VM-name QMP fallback risk.
- Use absolute dump paths and keep dump directories under tight ownership.
- Prefer explicit filesystem and socket mounts over broad container privilege.
- Keep JSONL output and memory dumps outside guest-writable paths.

## Current hardening in repo

- Strict config validation at startup.
- Invalid regex/category/severity/action fails startup.
- `validate-config` command for deployment checks.
- Health-aware sensor events and Prometheus health gauge.
- Buffered JSONL writes instead of flush-on-every-line churn.
- Control-plane collector messages do not use the lossy telemetry queue.
- QMP actions refuse VM-name fallback by default when no stable `vm_id` mapping matches.
- Policy action events record structured decision, retry, timeout, refusal, and bounded failure-class metadata.
- Safer dump path validation: absolute paths, no `..`, existing final symlink rejection, symlink ancestor rejection, `dump_root` safety checks, and canonical containment under `dump_root`.
- systemd unit with `NoNewPrivileges=yes`, filesystem restrictions, and explicit supplementary groups.
- Optional seccomp profile at `packaging/seccomp/aegishv-seccomp.json`; it is shipped as an operator-reviewed profile and is not enabled by default.
- Optional AppArmor profile at `packaging/apparmor/usr.bin.aegishv`; it is shipped as an operator-reviewed profile and is not enabled by default.
- Optional SELinux policy skeleton at `packaging/selinux`; it is shipped for operator review, includes common tracefs/debugfs read labels, and is not installed with `semodule` by default.
- cargo-deny policy in `deny.toml` for advisories, yanked crates, duplicate versions, wildcard dependency requirements, licenses, registries, and git sources.
- Release workflow with checksums, SBOM generation, Sigstore bundle signing, and GitHub SLSA provenance attestations.
- Event output redaction policy in `docs/EVENT_REDACTION.md`; runtime redaction is not implemented.
- AMD SEV, SEV-ES, and SEV-SNP are treated as confidentiality and integrity boundaries. The SVM lab model must report degraded or unsupported visibility when encrypted guest state prevents inspection; it must not claim a bypass.
- ARM64 pKVM, Arm CCA, and protected guest modes are treated as confidentiality and integrity boundaries. The EL2 lab model must report degraded or unsupported visibility when protected guest memory prevents inspection; it must not claim introspection for protected memory.
- Device assignment requires a proven DMA isolation path. Missing VT-d, AMD-Vi, SMMU, interrupt remapping, or fault reporting must refuse assignment rather than falling back to trust.

## Still missing for a regulated deployment

- Signed policy bundles.
- mTLS/authn/authz for remote outputs.
- Tamper-evident local audit log.
- Runtime event redaction controls.
- Live encrypted-guest handling policy for SEV, SEV-ES, SEV-SNP, TDX, and similar protections.
- Live protected-guest handling policy for pKVM, Arm CCA, and vendor ARM64 protected guest modes.
- Live virtual switch, SR-IOV, and passthrough enforcement are not implemented.
- Full secret-management story.
- Reproducible build verification beyond CI.
- Third-party dependency license review beyond cargo-deny's configured checks.
- Enforced AppArmor policy and deployment-specific seccomp tuning.
- Reviewed and enforced SELinux policy for the target distribution.
- Encrypted and hashed memory dump completion tracking.
