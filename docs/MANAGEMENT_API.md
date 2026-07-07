# Management Interface

AegisHV exposes a local management surface through the `aegishv admin` CLI. It is intentionally small and file based. The current code does not start a management daemon, does not expose an HTTP listener, and does not open a remote control port.

## Commands

```bash
aegishv version --json
aegishv admin health --json
aegishv admin policy-explain --config ./config.example.toml --json
aegishv admin policy-test --config ./config.example.toml --category wx --severity high --reason "sample" --vm vm-a --json
aegishv admin action-dry-run --config ./config.example.toml --kind pause_vm --vm vm-a --json
```

`policy-test` forces policy rules into dry-run mode for the simulated event. It is for reviewing match behavior before a config file is used by the collector.

`action-dry-run` exercises action validation and audit shaping without opening a QMP connection. It should be used before enabling any rule with an enforcing action.

## Access Model

The library includes a role and permission table for operator tooling:

- `viewer` can inspect health and policy state.
- `operator` can request pause, resume, and network quarantine actions.
- `incident_responder` can request guest-memory dump actions.
- `admin` can update policy material and approve actions.

This role table is not wired to a network identity provider. Callers that embed it must provide their own authenticated actor identity before checking permissions.

## Audit Records

`AppendOnlyAuditLog` appends bounded JSONL records and refuses symlink targets at open time. It does not replace operating-system file permissions, immutable file flags, or external log shipping. Operators should keep the audit path outside world-writable directories and rotate it with local policy.

## Manual Approvals

`ApprovalStore` writes pending action records and later decision records into an existing directory. Approval IDs are restricted to letters, digits, `-`, and `_` so a request cannot select an arbitrary path. The store is a local primitive for higher-level tooling; it does not authenticate the actor by itself.

## Evidence State

`DumpEvidence` keeps QMP acceptance separate from dump completion. A dump request is not treated as completed until the expected path exists and the recorded path matches the accepted request. The completion record includes a bounded content digest helper. That digest is a change detector, not a cryptographic signature.

## Operational Boundary

The current management surface is suitable for local review, dry runs, and integration into a guarded wrapper. Remote management, multi-user authentication, durable approval workflows, and hardware-backed attestation remain separate gates before stronger deployment claims.
