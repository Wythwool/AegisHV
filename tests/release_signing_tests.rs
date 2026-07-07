use std::fs;
use std::path::Path;

fn read_repo_file(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

#[test]
fn release_workflow_installs_cosign_and_uploads_signature_bundles() {
    let workflow = read_repo_file(".github/workflows/release.yml");

    for required in [
        "id-token: write",
        "COSIGN_VERSION: v3.0.5",
        "uses: sigstore/cosign-installer@v4.1.0",
        "cosign-release: ${{ env.COSIGN_VERSION }}",
        "cosign version",
        "bash ./scripts/sign-release-artifacts.sh ${{ matrix.target }}",
        "dist/*.tar.gz.sigstore.json",
        "dist/*.sbom.cdx.json.sigstore.json",
        "dist/SHA256SUMS-*.txt.sigstore.json",
    ] {
        assert!(
            workflow.contains(required),
            "release workflow is missing signing wiring: {required}"
        );
    }

    assert!(
        workflow.find("name: Checksums").expect("checksums step")
            < workflow
                .find("name: Sign release artifacts")
                .expect("signing step"),
        "release workflow must sign after checksum files are created"
    );

    for forbidden in ["COSIGN_PRIVATE_KEY", "secrets.COSIGN", "echo signature"] {
        assert!(
            !workflow.contains(forbidden),
            "release workflow must not contain fake or key-based signing material: {forbidden}"
        );
    }
}

#[test]
fn signing_script_uses_cosign_blob_bundles_for_artifacts_and_sbom() {
    let script = read_repo_file("scripts/sign-release-artifacts.sh");

    for required in [
        "command -v cosign",
        "cosign sign-blob --yes --bundle",
        "aegishv-${VERSION}-${TARGET}.tar.gz",
        "aegishv-${VERSION}-${TARGET}.sbom.cdx.json",
        "SHA256SUMS-${TARGET}.txt",
        ".sigstore.json",
        "cosign did not create a signature bundle",
    ] {
        assert!(
            script.contains(required),
            "signing script is missing required artifact signing guard: {required}"
        );
    }

    for forbidden in [
        "COSIGN_PRIVATE_KEY",
        "PRIVATE KEY",
        "-----BEGIN",
        "checksum-only",
    ] {
        assert!(
            !script.contains(forbidden),
            "signing script must not contain private keys or checksum-only signing: {forbidden}"
        );
    }
}

#[test]
fn release_docs_explain_keyless_verification_and_limits() {
    let release = read_repo_file("RELEASE.md");
    let testing = read_repo_file("docs/TESTING.md");

    for required in [
        "Sigstore",
        "Cosign `v3.0.5`",
        "id-token: write",
        "cosign verify-blob",
        "--certificate-identity https://github.com/Nullbit1/AegisHV/.github/workflows/release.yml@refs/tags/v0.4.0",
        "--certificate-oidc-issuer https://token.actions.githubusercontent.com",
        "No private signing key is committed",
        "checksums, not signatures",
        "do not prove reproducible builds",
    ] {
        assert!(
            release.contains(required),
            "RELEASE.md is missing signing documentation: {required}"
        );
    }

    for required in [
        "Release Signing Checks",
        "keyless Sigstore signing",
        "scripts/sign-release-artifacts.sh",
        "cosign sign-blob --yes --bundle",
        "*.sigstore.json",
        "Checksums are still generated, but checksums are not signatures",
    ] {
        assert!(
            testing.contains(required),
            "docs/TESTING.md is missing signing test documentation: {required}"
        );
    }
}

#[test]
fn security_doc_no_longer_lists_release_signing_as_missing() {
    let security = read_repo_file("docs/SECURITY.md");

    assert!(
        security.contains("Sigstore bundle signing"),
        "security doc must list release signing as current release hardening"
    );
    assert!(
        !security.contains("- Signed release artifacts."),
        "security doc must not still list signed release artifacts as missing"
    );
}
