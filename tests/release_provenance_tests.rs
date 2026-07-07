use std::fs;
use std::path::Path;

fn read_repo_file(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

#[test]
fn release_workflow_generates_and_uploads_slsa_provenance_bundle() {
    let workflow = read_repo_file(".github/workflows/release.yml");

    for required in [
        "attestations: write",
        "artifact-metadata: write",
        "run: bash ./scripts/package-release.sh ${{ matrix.target }}",
        "name: Generate SLSA provenance",
        "uses: actions/attest@v4.1.0",
        "subject-checksums: dist/SHA256SUMS-${{ matrix.target }}.txt",
        "steps.provenance.outputs.bundle-path",
        "bash ./scripts/collect-provenance-bundle.sh ${{ matrix.target }}",
        "dist/*.slsa-provenance.sigstore.json",
    ] {
        assert!(
            workflow.contains(required),
            "release workflow is missing provenance wiring: {required}"
        );
    }

    assert!(
        workflow.find("name: Checksums").expect("checksums step")
            < workflow
                .find("name: Generate SLSA provenance")
                .expect("provenance step"),
        "release workflow must create checksums before provenance subjects are selected"
    );
    assert!(
        workflow
            .find("name: Generate SLSA provenance")
            .expect("provenance step")
            < workflow
                .find("name: Sign release artifacts")
                .expect("signing step"),
        "release workflow must keep provenance separate from later blob signing"
    );

    for forbidden in ["provenance.json", "printf '{", "echo '{", "fake provenance"] {
        assert!(
            !workflow.contains(forbidden),
            "release workflow must not hand-write provenance: {forbidden}"
        );
    }
}

#[test]
fn provenance_bundle_collector_copies_action_output_and_rejects_host_paths() {
    let script = read_repo_file("scripts/collect-provenance-bundle.sh");

    for required in [
        "actions/attest did not report a provenance bundle path",
        "provenance bundle is missing or empty",
        "cp \"$BUNDLE\" \"$OUT\"",
        ".slsa-provenance.sigstore.json",
        "json.loads(text)",
        "mediaType",
        "dsseEnvelope",
        "/target/",
        "/.git/",
        ".pytest_cache",
        "__pycache__",
        "node_modules",
        "forbidden host/build path fragment",
    ] {
        assert!(
            script.contains(required),
            "provenance collect script is missing required guard: {required}"
        );
    }

    for forbidden in [
        "PRIVATE KEY",
        "-----BEGIN",
        "predicate = '{",
        "fake attestation",
    ] {
        assert!(
            !script.contains(forbidden),
            "provenance collect script must not contain static attestations or secrets: {forbidden}"
        );
    }
}

#[test]
fn release_docs_explain_provenance_verification_and_limits() {
    let release = read_repo_file("RELEASE.md");
    let testing = read_repo_file("docs/TESTING.md");

    for required in [
        "Verify SLSA provenance",
        "actions/attest@v4.1.0",
        "subject-checksums: dist/SHA256SUMS-x86_64-unknown-linux-gnu.txt",
        "scripts/collect-provenance-bundle.sh",
        "*.slsa-provenance.sigstore.json",
        "gh attestation verify",
        "--predicate-type https://slsa.dev/provenance/v1",
        "checksum file itself is signed separately but is not a SLSA provenance subject",
        "does not prove reproducible builds",
    ] {
        assert!(
            release.contains(required),
            "RELEASE.md is missing provenance documentation: {required}"
        );
    }

    for required in [
        "Release Provenance Checks",
        "GitHub artifact attestations",
        "scripts/collect-provenance-bundle.sh",
        "the release job fails",
        "Provenance is separate from checksums and Cosign blob signatures",
    ] {
        assert!(
            testing.contains(required),
            "docs/TESTING.md is missing provenance test documentation: {required}"
        );
    }
}

#[test]
fn security_doc_lists_provenance_as_current_release_hardening() {
    let security = read_repo_file("docs/SECURITY.md");

    assert!(
        security.contains("GitHub SLSA provenance attestations"),
        "security doc must list provenance as current release hardening"
    );
}
