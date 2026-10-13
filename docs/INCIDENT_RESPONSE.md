# Investigating an alert

Keep the raw JSONL, spool files, active config, binary version, and action audit
records. Check loss metadata before treating the stream as complete.

Run `aegishv admin policy-explain --config config.example.toml --json` against the
actual config. Distinguish dry runs, approvals, refusals, successful actions, and
accepted dump requests. For a dump, verify the expected file separately.

Identity, path, and unsupported-action refusals explain why dispatch stopped.
Review those reasons before changing a rule. Test changed policies with
`policy-test` and `action-dry-run` before enabling enforcement again.

Redact copies for sharing and retain originals under restricted access.
Offline VMI findings depend on the supplied snapshot and profile; they do not
establish the current state of a running guest.
