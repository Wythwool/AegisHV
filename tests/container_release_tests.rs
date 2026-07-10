use std::fs;
use std::path::Path;

fn read_repo_file(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

#[test]
fn dockerfile_declares_bounded_oci_labels() {
    let dockerfile = read_repo_file("Dockerfile");

    for required in [
        "ARG AEGISHV_VERSION=\"0.4.0\"",
        "ARG AEGISHV_REVISION=\"unknown\"",
        "ARG AEGISHV_CREATED=\"1970-01-01T00:00:00Z\"",
        "org.opencontainers.image.title=\"AegisHV\"",
        "org.opencontainers.image.description=\"Host-side KVM telemetry sensor\"",
        "org.opencontainers.image.source=\"https://github.com/Nullbit1/AegisHV\"",
        "org.opencontainers.image.url=\"https://github.com/Nullbit1/AegisHV\"",
        "org.opencontainers.image.documentation=\"https://github.com/Nullbit1/AegisHV\"",
        "org.opencontainers.image.version=\"${AEGISHV_VERSION}\"",
        "org.opencontainers.image.revision=\"${AEGISHV_REVISION}\"",
        "org.opencontainers.image.created=\"${AEGISHV_CREATED}\"",
        "org.opencontainers.image.licenses=\"MIT\"",
        "org.opencontainers.image.authors=\"https://github.com/Wythwool\"",
        "org.opencontainers.image.vendor=\"https://github.com/Nullbit1\"",
    ] {
        assert!(
            dockerfile.contains(required),
            "Dockerfile is missing required OCI label wiring: {required}"
        );
    }
}

#[test]
fn dockerignore_excludes_build_outputs_and_local_artifacts() {
    let dockerignore = read_repo_file(".dockerignore");

    for required in [
        "target",
        ".git",
        "dist",
        ".pytest_cache",
        "__pycache__",
        "node_modules",
        ".cache",
        "*.zip",
        "*.tar.gz",
        "*.rpm",
        "*.deb",
    ] {
        assert!(
            dockerignore.lines().any(|line| line == required),
            ".dockerignore is missing required build-context exclusion: {required}"
        );
    }
}

#[test]
fn container_docs_state_current_publish_and_signing_limits() {
    let deployment = read_repo_file("docs/DEPLOYMENT.md");
    let testing = read_repo_file("docs/TESTING.md");
    let release = read_repo_file("RELEASE.md");

    for required in [
        "There is no current release workflow that publishes or signs container images",
        "The Dockerfile sets bounded OCI image labels",
        "AEGISHV_VERSION",
        "AEGISHV_REVISION",
        "AEGISHV_CREATED",
        ".dockerignore",
        "Container signing is documentation-only in this tree",
        "It is not evidence that a current AegisHV container image exists or is signed",
        "Container signing would bind an image digest to a CI identity",
        "would not prove runtime confinement",
    ] {
        assert!(
            deployment.contains(required),
            "docs/DEPLOYMENT.md is missing container label/signing limit: {required}"
        );
    }

    for required in [
        "Container Metadata Checks",
        "Normal PR tests do not publish or sign container images",
        "Docker build smoke",
        "standard OCI label keys",
        "real Sigstore/Cosign signing of the image digest",
        "Checksums or text files are not container signatures",
    ] {
        assert!(
            testing.contains(required),
            "docs/TESTING.md is missing container metadata test documentation: {required}"
        );
    }

    for required in [
        "Verify container image labels if building a local image",
        "The current release workflow does not publish or sign container images",
        "sign the pushed image digest with real Sigstore/Cosign keyless signing",
        "cosign verify ghcr.io/nullbit1/aegishv:0.4.0",
        "would not prove runtime confinement",
    ] {
        assert!(
            release.contains(required),
            "RELEASE.md is missing container label/signing release guidance: {required}"
        );
    }
}

#[test]
fn release_workflow_has_no_fake_container_publish_or_signing_path() {
    let release_workflow = read_repo_file(".github/workflows/release.yml");
    let ci_workflow = read_repo_file(".github/workflows/ci.yml");

    assert!(
        ci_workflow.contains("Docker build smoke"),
        "CI should keep the existing Docker build smoke"
    );

    for forbidden in [
        "docker/login-action",
        "docker push",
        "ghcr.io/nullbit1/aegishv",
        "cosign sign ghcr.io",
        "cosign sign --yes ghcr.io",
        "CONTAINER_PRIVATE_KEY",
        "PRIVATE KEY",
        "echo signed image",
    ] {
        assert!(
            !release_workflow.contains(forbidden),
            "release workflow must not claim fake container publishing or signing: {forbidden}"
        );
    }
}

#[test]
fn ci_bounds_docker_registry_retries_and_uses_node24_actions() {
    let ci_workflow = read_repo_file(".github/workflows/ci.yml");

    for required in [
        "actions/checkout@v7",
        "actions/upload-artifact@v7",
        "timeout-minutes: 10",
        "for attempt in 1 2 3",
        "docker build -t aegishv:ci .",
        "Docker build failed after ${attempt} attempts",
        "retrying in ${delay}s",
    ] {
        assert!(
            ci_workflow.contains(required),
            "CI is missing Docker retry or Node 24 action wiring: {required}"
        );
    }

    assert!(
        !ci_workflow.contains("actions/checkout@v4")
            && !ci_workflow.contains("actions/upload-artifact@v4"),
        "normal CI should not use the deprecated Node 20 action majors"
    );
}
