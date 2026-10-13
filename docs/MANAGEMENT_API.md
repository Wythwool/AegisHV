# Local administration

```sh
aegishv version --json
aegishv admin health --json
aegishv admin policy-explain --config config.example.toml --json
aegishv admin policy-test --config config.example.toml --category wx \
  --severity high --reason sample --vm vm-a --json
aegishv admin action-dry-run --config config.example.toml --kind pause_vm \
  --vm vm-a --json
```

`policy-test` forces dry-run evaluation. `action-dry-run` validates an action and
its audit result without connecting to QMP. There is no remote management daemon.

Library support includes a role table (`viewer`, `operator`,
`incident_responder`, `admin`), a file-backed `ApprovalStore`, and an
`AppendOnlyAuditLog`. Embedding code must authenticate actors and enforce roles;
the files alone do not do that. Audit files refuse symlink targets on open.
Approval IDs permit letters, digits, `-`, and `_`.

`DumpEvidence` distinguishes QMP acceptance from completion at the expected
path. Its content digest is a change detector, not a signature. Keep audit,
approval, and evidence directories under restrictive OS permissions.
