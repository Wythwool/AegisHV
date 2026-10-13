# Releases

Build and test the committed source with the pinned toolchain:

```sh
cargo test --locked --all --all-features
cargo build --locked --release
./scripts/smoke-replay.sh
./scripts/check-doc-links.sh
bash scripts/package-release.sh x86_64-unknown-linux-gnu
```

The package script writes to `dist`. Use the matching target binary when making
a cross-compiled package. The `v*` tag workflow builds x86_64 and aarch64 Linux
archives for glibc and musl using `cross`.

Each target also gets a CycloneDX SBOM from Syft, SHA-256 checksums, SLSA
provenance from GitHub Actions, and keyless Sigstore bundles from Cosign.
The scripts are `generate-sbom.sh`, `collect-provenance-bundle.sh`, and
`sign-release-artifacts.sh`. The workflow pins tool versions.

## Verify downloaded artifacts

Check the archive against its `SHA256SUMS-*.txt` file. Verify Sigstore bundles
against this repository's release workflow identity and the GitHub Actions
OIDC issuer (`https://token.actions.githubusercontent.com`). For example, set
`release_tag` to the tag being checked:

```sh
release_tag=v0.4.0
artifact=aegishv-0.4.0-x86_64-unknown-linux-gnu.tar.gz
cosign verify-blob --bundle "$artifact.sigstore.json" \
  --certificate-identity "https://github.com/Wythwool/AegisHV/.github/workflows/release.yml@refs/tags/$release_tag" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  "$artifact"
gh attestation verify "$artifact" --repo Wythwool/AegisHV
```

An SBOM lists package contents; it is not a vulnerability scan. Provenance and
signatures identify build origin and artifact bytes. They do not certify host
compatibility or authenticate runtime events.

The normal release contains the userspace sensor. Kernel images need separate
[boot evidence](docs/TYPE1_BOOT_BOUNDARY.md). No live hardware run is implied by
a successful cross-build.
