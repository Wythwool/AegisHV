use std::fmt;
use std::fs;
use std::path::Path;

const CREATOR: &str = "https://github.com/Wythwool";
const ORGANIZATION: &str = "https://github.com/Nullbit1";

#[derive(Debug, PartialEq, Eq)]
enum OwnershipDocError {
    MissingFile(&'static str),
    MissingCreator(&'static str),
    MissingOrganization(&'static str),
    GenericAuthorPlaceholder,
    AngleBracketPlaceholder(&'static str),
    FakeLegalClaim(&'static str),
}

impl fmt::Display for OwnershipDocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OwnershipDocError::MissingFile(path) => write!(f, "{path} is missing"),
            OwnershipDocError::MissingCreator(path) => write!(f, "{path} is missing creator link"),
            OwnershipDocError::MissingOrganization(path) => {
                write!(f, "{path} is missing organization link")
            }
            OwnershipDocError::GenericAuthorPlaceholder => {
                write!(f, "Cargo.toml still uses a generic author placeholder")
            }
            OwnershipDocError::AngleBracketPlaceholder(path) => {
                write!(f, "{path} contains an angle-bracket placeholder")
            }
            OwnershipDocError::FakeLegalClaim(path) => {
                write!(f, "{path} adds a legal ownership claim outside the license")
            }
        }
    }
}

fn read_required_file(root: &Path, rel: &'static str) -> Result<String, OwnershipDocError> {
    fs::read_to_string(root.join(rel)).map_err(|_| OwnershipDocError::MissingFile(rel))
}

fn validate_ownership_text(path: &'static str, text: &str) -> Result<(), OwnershipDocError> {
    if !text.contains(CREATOR) {
        return Err(OwnershipDocError::MissingCreator(path));
    }
    if !text.contains(ORGANIZATION) {
        return Err(OwnershipDocError::MissingOrganization(path));
    }
    if text.contains('<') || text.contains('>') {
        return Err(OwnershipDocError::AngleBracketPlaceholder(path));
    }

    let lower = text.to_ascii_lowercase();
    for phrase in [
        "copyright assigned",
        "all rights reserved",
        "legal owner",
        "official support",
    ] {
        if lower.contains(phrase) {
            return Err(OwnershipDocError::FakeLegalClaim(path));
        }
    }

    Ok(())
}

fn validate_cargo_metadata(text: &str) -> Result<(), OwnershipDocError> {
    validate_ownership_text("Cargo.toml", text)?;
    if text.contains("AegisHV Authors") {
        return Err(OwnershipDocError::GenericAuthorPlaceholder);
    }
    for required in [
        "authors = [\"Wythwool\", \"Nullbit1\"]",
        "[package.metadata.aegishv.ownership]",
        "creator = \"https://github.com/Wythwool\"",
        "organization = \"https://github.com/Nullbit1\"",
    ] {
        if !text.contains(required) {
            return Err(OwnershipDocError::MissingCreator("Cargo.toml"));
        }
    }
    Ok(())
}

fn readme_ownership_section(text: &str) -> &str {
    let Some((_, rest)) = text.split_once("## Ownership Metadata") else {
        return "";
    };
    rest.split_once("\n## ")
        .map(|(section, _)| section)
        .unwrap_or(rest)
}

#[test]
fn ownership_metadata_uses_concrete_links_without_placeholders() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo = read_required_file(root, "Cargo.toml").expect("Cargo.toml must exist");
    let readme = read_required_file(root, "README.md").expect("README.md must exist");
    let release = read_required_file(root, "RELEASE.md").expect("RELEASE.md must exist");

    validate_cargo_metadata(&cargo).expect("Cargo ownership metadata must be concrete");
    validate_ownership_text("README.md", readme_ownership_section(&readme))
        .expect("README ownership metadata must be concrete");
    validate_ownership_text("RELEASE.md", &release)
        .expect("release checklist must keep ownership links");
}

#[test]
fn ownership_validator_rejects_missing_creator() {
    assert_eq!(
        validate_ownership_text("README.md", ORGANIZATION),
        Err(OwnershipDocError::MissingCreator("README.md"))
    );
}

#[test]
fn ownership_validator_rejects_angle_bracket_placeholder() {
    let text = format!("Creator: {CREATOR}\nOrganization: {ORGANIZATION}\nOwner: <owner>\n");

    assert_eq!(
        validate_ownership_text("README.md", &text),
        Err(OwnershipDocError::AngleBracketPlaceholder("README.md"))
    );
}

#[test]
fn cargo_metadata_validator_rejects_generic_author_placeholder() {
    let text = format!(
        "authors = [\"AegisHV Authors\"]\ncreator = \"{CREATOR}\"\norganization = \"{ORGANIZATION}\"\n"
    );

    assert_eq!(
        validate_cargo_metadata(&text),
        Err(OwnershipDocError::GenericAuthorPlaceholder)
    );
}

#[test]
fn ownership_validator_rejects_fake_legal_claims() {
    let text = format!(
        "Creator: {CREATOR}\nOrganization: {ORGANIZATION}\nOfficial support is included.\n"
    );

    assert_eq!(
        validate_ownership_text("README.md", &text),
        Err(OwnershipDocError::FakeLegalClaim("README.md"))
    );
}
