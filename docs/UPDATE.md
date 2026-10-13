# Updating

Build with `--locked`, run tests and replay, and verify the release artifacts
when using a downloaded binary. Keep the previous binary and config until the
replacement passes startup and health checks.

Stop the service, replace the binary using the host's package/file controls,
validate the config, then start it. Record `aegishv version --json`.

For policy changes, use `admin policy-explain`, `admin policy-test`, and
`admin action-dry-run`. Signed bundle handling requires a caller-supplied trust
root; see [policy bundles](POLICY_BUNDLES.md). Unix config reload and its
startup-only exceptions are listed in [deployment](DEPLOYMENT.md).

There is no automatic update or rollback service.
