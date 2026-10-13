# Build identity

`aegishv version --json` reports package version, OS, architecture, and the Git
revision supplied at compile time through `AEGISHV_BUILD_GIT_REV`. An unset
revision is `unknown`.

This metadata is informational. Release provenance and Sigstore verification
are described in [RELEASE.md](../RELEASE.md). The project has no TPM quote
verifier, measured boot, or remote-attestation service.

`src/tamper.rs` provides local file-change digests. Those digests detect changes;
they do not authenticate the host or binary.
