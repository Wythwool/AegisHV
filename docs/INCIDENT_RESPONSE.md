# Incident Response

AegisHV can emit structured events, policy audit events, and local action dry-run results. The current repository does not replace an incident-response platform.

## First Checks

1. Preserve the JSONL event stream, spool directory, config file, and binary version.
2. Run `aegishv version --json` and record the output.
3. Run `aegishv admin policy-explain --config FILE --json` against the active config.
4. Confirm whether any action event was `dry_run`, `manual_approval`, `refused`, or `success`.
5. For dump actions, check the separate accepted and completed evidence states.

## Action Handling

Actions should be treated as operator decisions. A QMP action refusal is evidence that a safety gate fired, not a runtime failure by itself. Common refusal reasons include missing stable VM identity, unsafe dump path, unsupported action kind, and configured dry-run mode.

## Evidence Handling

Keep event JSONL, action audit records, approval decisions, and dump evidence in the same case folder when possible. Do not edit raw event files during triage. If records must be redacted for sharing, keep the original under restricted access and document the redaction step.

## Recovery

Prefer config changes that reduce action scope before restarting collection. Use `policy-test` and `action-dry-run` for changed rules before enabling enforcement again.

## Boundary

The current code does not acquire guest memory from a live VMI backend, does not prove live guest state, and does not provide a remote case-management workflow.
