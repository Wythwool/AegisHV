use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn doc_link_checker_runs_when_bash_is_available() {
    let output = match Command::new("bash")
        .current_dir(repo_root())
        .arg("scripts/check-doc-links.sh")
        .output()
    {
        Ok(output) => output,
        Err(err) if err.kind() == ErrorKind::NotFound => return,
        Err(err) => panic!("run bash: {err}"),
    };

    assert!(
        output.status.success(),
        "doc link check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("checked markdown links"));
}
