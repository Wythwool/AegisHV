# Release checklist

1. Update `Cargo.toml` version and `CHANGELOG.md`.
2. Verify ownership metadata uses concrete links:

   - Creator: https://github.com/Wythwool
   - Organization: https://github.com/Nullbit1

3. Verify the committed lockfile:

   ```bash
   cargo metadata --locked --format-version 1
   ./scripts/check-lockfile.sh
   ```

4. Verify dependency policy when cargo-deny is installed:

   ```bash
   cargo deny check
   ```

   `deny.toml` covers RustSec advisories, yanked crates, duplicate crate versions, wildcard dependency requirements, explicit license allowlists, unknown registries, and unknown git sources. The current locked graph has no third-party crates. If a release adds dependencies, review their license/source/advisory state and update the policy intentionally. AegisHV's MIT package license does not approve future dependency licenses by itself.

5. Run local gates:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --locked --all-targets --all-features -- -D warnings
   cargo test --locked --all --all-features
   ./scripts/smoke-replay.sh
   ```

6. Build release artifacts:

   ```bash
   cargo build --locked --release
   bash ./scripts/package-release.sh x86_64-unknown-linux-gnu
   bash ./scripts/package-debian.sh x86_64-unknown-linux-gnu
   bash ./scripts/package-rpm.sh x86_64-unknown-linux-gnu
   bash ./scripts/generate-sbom.sh x86_64-unknown-linux-gnu
   sha256sum "dist/aegishv-0.4.0-x86_64-unknown-linux-gnu.tar.gz" \
     "dist/aegishv-0.4.0-x86_64-unknown-linux-gnu.sbom.cdx.json" \
     | tee dist/SHA256SUMS-x86_64-unknown-linux-gnu.txt
   bash ./scripts/sign-release-artifacts.sh x86_64-unknown-linux-gnu
   ```

7. Verify SBOM generation:

   - release workflow installs Syft `v1.18.1`;
   - `scripts/generate-sbom.sh` scans the per-target package directory, such as `dist/aegishv-0.4.0-x86_64-unknown-linux-gnu`;
   - output uses the same target suffix, such as `dist/aegishv-0.4.0-x86_64-unknown-linux-gnu.sbom.cdx.json`;
   - the release package includes `Cargo.toml` and `Cargo.lock` so the locked Rust dependency graph is visible to the SBOM tool.

   If Syft is unavailable, SBOM generation must fail. Do not replace that failure with a handwritten dependency list.

8. Verify Debian packaging when releasing Debian artifacts:

   - `packaging/debian/control` is the package metadata;
   - `packaging/debian/install` records the installed layout;
   - `scripts/package-debian.sh x86_64-unknown-linux-gnu` builds `dist/aegishv_0.4.0_amd64.deb`;
   - `scripts/package-debian.sh aarch64-unknown-linux-gnu` builds `dist/aegishv_0.4.0_arm64.deb`.

   The package installs the binary, config, schemas, systemd unit, tmpfiles config, operator scripts, and docs. It creates the `aegishv` system user/group and runtime directories with mode `0750`. It does not enable or start the service automatically. Debian packaging does not imply type-1, VMI, EPT/NPT enforcement, syscall integrity, hardware PMU, or live libvirt support.

9. Verify RPM packaging when releasing RPM artifacts:

   - `packaging/rpm/aegishv.spec` is the package metadata;
   - `packaging/rpm/aegishv.service` is the systemd unit;
   - `packaging/rpm/aegishv.tmpfiles` creates runtime directories;
   - `scripts/package-rpm.sh x86_64-unknown-linux-gnu` builds an `x86_64` RPM under `dist/`;
   - `scripts/package-rpm.sh aarch64-unknown-linux-gnu` builds an `aarch64` RPM under `dist/`.

   The package installs the binary, config, schemas, systemd unit, tmpfiles config, operator scripts, and docs. It creates the `aegishv` system user/group and runtime directories with mode `0750`. It does not enable or start the service automatically. RPM packaging does not imply type-1, VMI, EPT/NPT enforcement, syscall integrity, hardware PMU, live libvirt support, package signing, or guaranteed delivery.

10. Verify container image labels if building a local image:

   - the Dockerfile sets OCI labels for title, description, source, revision, version, created timestamp, licenses, authors, and vendor;
   - `.dockerignore` excludes build outputs, repository metadata, `dist/`, caches, package outputs, and old archives from the build context;
   - release builds should pass `AEGISHV_VERSION`, `AEGISHV_REVISION`, and `AEGISHV_CREATED` as concrete build arguments.

   Local label check:

   ```bash
   docker build \
     --build-arg AEGISHV_VERSION=0.4.0 \
     --build-arg AEGISHV_REVISION=$(git rev-parse HEAD) \
     --build-arg AEGISHV_CREATED=$(date -u +%Y-%m-%dT%H:%M:%SZ) \
     -t aegishv:0.4.0 .
   docker image inspect aegishv:0.4.0 --format '{{ index .Config.Labels "org.opencontainers.image.source" }}'
   docker image inspect aegishv:0.4.0 --format '{{ index .Config.Labels "org.opencontainers.image.revision" }}'
   ```

   The current release workflow does not publish or sign container images. If image publishing is added later, sign the pushed image digest with real Sigstore/Cosign keyless signing. Verification should use the published image digest, the release workflow identity, and the Sigstore issuer:

   ```bash
   cosign verify ghcr.io/nullbit1/aegishv:0.4.0 \
     --certificate-identity https://github.com/Nullbit1/AegisHV/.github/workflows/release.yml@refs/tags/v0.4.0 \
     --certificate-oidc-issuer https://token.actions.githubusercontent.com
   ```

   Container signing would bind an image digest to the CI identity. It would not prove runtime confinement, safe host mounts, vulnerability status, reproducible builds, source review quality, type-1 support, VMI, EPT/NPT enforcement, syscall integrity, live libvirt support, hardware PMU support, or that the registry and CI provider were uncompromised.

11. Verify SLSA provenance:

   - release workflow grants `attestations: write` and `artifact-metadata: write`;
   - release workflow uses `actions/attest@v4.1.0` after the target checksum file is created;
   - `subject-checksums: dist/SHA256SUMS-x86_64-unknown-linux-gnu.txt` identifies the release tarball and CycloneDX SBOM as attestation subjects;
   - `scripts/collect-provenance-bundle.sh` copies the action output bundle into `dist/`;
   - provenance bundles are uploaded as `*.slsa-provenance.sigstore.json` release files.

   Expected provenance bundle for `x86_64-unknown-linux-gnu`:

   - `dist/aegishv-0.4.0-x86_64-unknown-linux-gnu.slsa-provenance.sigstore.json`.

   Verify a downloaded artifact against the uploaded provenance bundle with GitHub CLI:

   ```bash
   gh attestation verify aegishv-0.4.0-x86_64-unknown-linux-gnu.tar.gz \
     -R Nullbit1/AegisHV \
     --bundle aegishv-0.4.0-x86_64-unknown-linux-gnu.slsa-provenance.sigstore.json \
     --predicate-type https://slsa.dev/provenance/v1
   ```

   Use the same command shape for the SBOM artifact. The provenance subject set comes from `SHA256SUMS-x86_64-unknown-linux-gnu.txt`; the checksum file itself is signed separately but is not a SLSA provenance subject.

12. Verify release signing:

   - release workflow grants `id-token: write` and uses GitHub OIDC for keyless signing;
   - release workflow installs `sigstore/cosign-installer@v4.1.0` with Cosign `v3.0.5`;
   - `scripts/sign-release-artifacts.sh` signs the release tarball, CycloneDX SBOM, and target checksum file;
   - signature bundles are uploaded as `*.sigstore.json` files next to the signed artifacts.

   No private signing key is committed or required for this keyless path. If AegisHV later moves to key-based signing, the private key must live outside the repository in a release secret or hardware/KMS-backed signer.

   Expected signature bundles for `x86_64-unknown-linux-gnu` are:

   - `dist/aegishv-0.4.0-x86_64-unknown-linux-gnu.tar.gz.sigstore.json`;
   - `dist/aegishv-0.4.0-x86_64-unknown-linux-gnu.sbom.cdx.json.sigstore.json`;
   - `dist/SHA256SUMS-x86_64-unknown-linux-gnu.txt.sigstore.json`.

   Verify a downloaded artifact with Cosign:

   ```bash
   cosign verify-blob aegishv-0.4.0-x86_64-unknown-linux-gnu.tar.gz \
     --bundle aegishv-0.4.0-x86_64-unknown-linux-gnu.tar.gz.sigstore.json \
     --certificate-identity https://github.com/Nullbit1/AegisHV/.github/workflows/release.yml@refs/tags/v0.4.0 \
     --certificate-oidc-issuer https://token.actions.githubusercontent.com
   ```

   Use the same command shape for the SBOM and `SHA256SUMS` files. The certificate identity must match the repository, workflow, and tag that created the release.

13. Tag the tested release version and let the release workflow build the target matrix.

No release workflow should generate or rewrite `Cargo.lock`. The source archive must already contain the lockfile that was tested.

The SBOM is a CycloneDX JSON document generated by Syft for each release target package. It is release evidence, not a vulnerability scan, license approval, provenance attestation, signature, or guarantee that downstream packaging did not change the binary.

SLSA provenance records the GitHub Actions workflow identity, source revision, and subject digests for the attested release tarball and SBOM. It does not prove reproducible builds, source review quality, vulnerability status, license approval, runtime behavior, or that GitHub Actions and the repository were uncompromised.

`SHA256SUMS-*` files are checksums, not signatures or provenance. They help detect accidental corruption after the checksum file itself is trusted. The Sigstore bundles bind each signed file to the GitHub Actions OIDC identity used by the release workflow and to Sigstore verification services. They do not prove reproducible builds, code review quality, vulnerability status, license approval, or that GitHub Actions, the repository, Fulcio, Rekor, or the operator's trust policy were uncompromised.

cargo-deny is a dependency policy gate. It does not prove vulnerability absence, license approval, supply-chain security, source review quality, or runtime safety.
