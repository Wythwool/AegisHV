# Update Notes

AegisHV updates are expected to be staged and verified before replacing a running binary or policy file.

## Binary Update Flow

1. Build with a locked dependency graph.
2. Run local tests and replay smoke.
3. Verify release checksums and signatures when using release artifacts.
4. Stop the service with the local service manager.
5. Replace the binary and config atomically according to host policy.
6. Run `aegishv version --json` and `aegishv validate-config --config FILE`.
7. Start the service and inspect health, logs, and replay output.

## Policy Update Flow

1. Verify the policy bundle signature with the local trust root.
2. Reject rollback versions.
3. Run `aegishv admin policy-explain --config FILE --json`.
4. Run targeted `aegishv admin policy-test` cases for changed rules.
5. Run `aegishv admin action-dry-run` for every enforcing action.
6. Write an audit record for the actor, bundle version, decision, and config digest.
7. Replace the active config through the host's normal file-permission controls.

## Rollback

Rollback is an operator action, not automatic behavior. Keep the previous binary, config, and evidence directory available until the replacement has passed replay and health checks. Policy bundle rollback still requires an explicit version decision and audit record.

## Boundary

There is no self-update mechanism. The binary does not download artifacts, update itself, or reload policy over a network channel.
