use std::fs;
use std::path::Path;

fn read_repo_file(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

#[test]
fn deny_policy_has_explicit_advisory_license_ban_and_source_sections() {
    let deny = read_repo_file("deny.toml");

    for required in [
        "[advisories]",
        "db-urls = [\"https://github.com/rustsec/advisory-db\"]",
        "yanked = \"deny\"",
        "ignore = []",
        "[licenses]",
        "confidence-threshold = 0.8",
        "exceptions = []",
        "[licenses.private]",
        "ignore = false",
        "registries = []",
        "[bans]",
        "multiple-versions = \"deny\"",
        "wildcards = \"deny\"",
        "highlight = \"all\"",
        "workspace-default-features = \"allow\"",
        "external-default-features = \"allow\"",
        "allow = []",
        "allow-workspace = false",
        "deny = []",
        "skip = []",
        "skip-tree = []",
        "[sources]",
        "unknown-registry = \"deny\"",
        "unknown-git = \"deny\"",
        "allow-registry = [\"https://github.com/rust-lang/crates.io-index\"]",
        "allow-git = []",
        "[sources.allow-org]",
        "github = []",
        "gitlab = []",
        "bitbucket = []",
    ] {
        assert!(
            deny.contains(required),
            "deny.toml is missing required dependency policy entry: {required}"
        );
    }

    for license in [
        "\"Apache-2.0\"",
        "\"BSD-2-Clause\"",
        "\"BSD-3-Clause\"",
        "\"ISC\"",
        "\"MIT\"",
        "\"Unicode-DFS-2016\"",
    ] {
        assert!(
            deny.contains(license),
            "deny.toml is missing allowed license: {license}"
        );
    }
}

#[test]
fn deny_policy_rejects_broad_allowlists_and_placeholders() {
    let deny = read_repo_file("deny.toml");

    for forbidden in [
        "multiple-versions = \"allow\"",
        "wildcards = \"allow\"",
        "unknown-registry = \"allow\"",
        "unknown-git = \"allow\"",
        "allow-osi-fsf-free",
        "allow-registry = []",
        "allow-git = [\"*\"]",
        "github = [\"*\"]",
        "gitlab = [\"*\"]",
        "bitbucket = [\"*\"]",
        "<",
        ">",
        "C:\\Users",
        "/Users/",
        "PRIVATE KEY",
        "BEGIN RSA",
    ] {
        assert!(
            !deny.contains(forbidden),
            "deny.toml contains broad, placeholder, or local-only policy text: {forbidden}"
        );
    }
}

#[test]
fn ci_runs_full_cargo_deny_check_without_private_services() {
    let ci = read_repo_file(".github/workflows/ci.yml");

    for required in [
        "name: dependency policy",
        "cargo install cargo-deny --locked",
        "cargo deny check",
    ] {
        assert!(
            ci.contains(required),
            "CI is missing cargo-deny dependency policy wiring: {required}"
        );
    }

    for forbidden in [
        "cargo deny check bans licenses advisories",
        "cargo deny check bans",
        "cargo deny check licenses",
        "cargo deny check advisories",
        "cargo deny check sources --allow",
        "secrets.",
        "SELINUX",
        "apparmor_parser",
        "live-tracefs-smoke.sh",
    ] {
        assert!(
            !ci.contains(forbidden),
            "CI dependency policy must not use partial checks, secrets, or host-only tooling: {forbidden}"
        );
    }
}

#[test]
fn dependency_policy_docs_state_scope_and_limits() {
    let testing = read_repo_file("docs/TESTING.md");
    let security = read_repo_file("docs/SECURITY.md");
    let release = read_repo_file("RELEASE.md");

    for required in [
        "Dependency Policy Checks",
        "cargo deny check",
        "RustSec advisories",
        "yanked crates",
        "duplicate crate versions",
        "wildcard dependency requirements",
        "license allowlists",
        "unknown registries",
        "unknown git sources",
        "current lockfile contains only the AegisHV crate",
        "AegisHV's MIT package license does not approve future dependency licenses",
        "does not prove vulnerability absence",
        "license approval",
        "supply-chain security",
    ] {
        assert!(
            testing.contains(required),
            "docs/TESTING.md is missing dependency policy guidance: {required}"
        );
    }

    for required in [
        "cargo-deny policy in `deny.toml`",
        "advisories, yanked crates, duplicate versions",
        "licenses, registries, and git sources",
        "Third-party dependency license review beyond cargo-deny's configured checks",
    ] {
        assert!(
            security.contains(required),
            "docs/SECURITY.md is missing dependency policy posture text: {required}"
        );
    }

    for required in [
        "Verify dependency policy when cargo-deny is installed",
        "cargo deny check",
        "`deny.toml` covers RustSec advisories",
        "current locked graph has no third-party crates",
        "AegisHV's MIT package license does not approve future dependency licenses",
        "cargo-deny is a dependency policy gate",
        "does not prove vulnerability absence",
    ] {
        assert!(
            release.contains(required),
            "RELEASE.md is missing dependency policy release guidance: {required}"
        );
    }
}
