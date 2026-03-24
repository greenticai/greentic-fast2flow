//! CLI integration tests for bundle commands.

use std::path::PathBuf;
use std::process::Command;

fn cargo_bin() -> PathBuf {
    // CARGO_MANIFEST_DIR is cli/fast2flow-cli, target is at workspace root
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../target/debug/greentic-fast2flow");
    path
}

fn sample_bundle_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/sample-bundle")
}

#[test]
fn cli_bundle_validate_succeeds() {
    let output = Command::new(cargo_bin())
        .args(["bundle", "validate", "--bundle"])
        .arg(sample_bundle_path())
        .output()
        .expect("Failed to execute CLI");

    assert!(
        output.status.success(),
        "bundle validate should succeed.\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("valid"));
}

#[test]
fn cli_bundle_validate_fails_on_empty_dir() {
    let empty_dir = tempfile::tempdir().unwrap();

    let output = Command::new(cargo_bin())
        .args(["bundle", "validate", "--bundle"])
        .arg(empty_dir.path())
        .output()
        .expect("Failed to execute CLI");

    assert!(
        !output.status.success(),
        "bundle validate should fail for empty dir"
    );
}

#[test]
fn cli_bundle_index_produces_output() {
    let output_dir = tempfile::tempdir().unwrap();

    let output = Command::new(cargo_bin())
        .args(["bundle", "index", "--bundle"])
        .arg(sample_bundle_path())
        .arg("--output")
        .arg(output_dir.path())
        .args(["--tenant", "cli-test", "--team", "default"])
        .output()
        .expect("Failed to execute CLI");

    assert!(
        output.status.success(),
        "bundle index should succeed.\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify index.json was created
    assert!(
        output_dir.path().join("index.json").exists(),
        "index.json should be created"
    );

    // Verify intents.md was created (--generate-docs defaults to true)
    assert!(
        output_dir.path().join("intents.md").exists(),
        "intents.md should be created"
    );

    // Verify stdout contains JSON manifest
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cli-test:default"));
}
