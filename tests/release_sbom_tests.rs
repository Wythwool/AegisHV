use std::fs;
use std::path::Path;

fn read_repo_file(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

#[test]
fn release_workflow_generates_sbom_with_syft_not_static_json() {
    let workflow = read_repo_file(".github/workflows/release.yml");

    for required in [
        "name: release",
        "jobs:",
        "steps:",
        "SYFT_VERSION: v1.18.1",
        "raw.githubusercontent.com/anchore/syft/${SYFT_VERSION}/install.sh",
        "syft version",
        "bash ./scripts/generate-sbom.sh ${{ matrix.target }}",
    ] {
        assert!(
            workflow.contains(required),
            "release workflow is missing required SBOM wiring: {required}"
        );
    }

    for forbidden in [
        "Minimal SBOM",
        "dependencies\":[]",
        "printf '{\"name\":\"aegishv\"",
    ] {
        assert!(
            !workflow.contains(forbidden),
            "release workflow still contains placeholder SBOM generation: {forbidden}"
        );
    }

    assert!(
        !workflow.contains('\t'),
        "release workflow should not contain tab indentation"
    );
}

#[test]
fn sbom_script_requires_real_tool_and_locked_release_inputs() {
    let script = read_repo_file("scripts/generate-sbom.sh");

    for required in [
        "command -v syft",
        "syft \"dir:$PKG\"",
        "cyclonedx-json=$OUT",
        "Cargo.toml",
        "Cargo.lock",
        "bomFormat",
        "CycloneDX",
        "/target/",
        "/.git/",
        ".pytest_cache",
        "__pycache__",
        "node_modules",
        "forbidden host/build path fragment",
    ] {
        assert!(
            script.contains(required),
            "SBOM script is missing required release guard: {required}"
        );
    }

    assert!(
        !script.contains("dependencies\":[]"),
        "SBOM script must not write a static empty dependency list"
    );
}

#[test]
fn release_package_includes_lockfile_for_sbom_scan() {
    let script = read_repo_file("scripts/package-release.sh");

    assert!(
        script.contains("Cargo.toml Cargo.lock"),
        "release package must include Cargo.toml and Cargo.lock for SBOM generation"
    );
}

#[test]
fn release_docs_describe_sbom_tool_path_and_limits() {
    let release = read_repo_file("RELEASE.md");
    let testing = read_repo_file("docs/TESTING.md");

    for (name, text) in [("RELEASE.md", release), ("docs/TESTING.md", testing)] {
        for required in [
            "Syft `v1.18.1`",
            "scripts/generate-sbom.sh",
            "CycloneDX",
            "Cargo.toml",
            "Cargo.lock",
            "If Syft is unavailable",
            "not a vulnerability scan",
        ] {
            assert!(
                text.contains(required),
                "{name} is missing required SBOM documentation: {required}"
            );
        }
    }
}
