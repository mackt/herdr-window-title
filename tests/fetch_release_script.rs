//! Exercises scripts/fetch-release.sh — the manifest's [[build]] step — in a
//! throwaway copy of the plugin root, against a fake release directory
//! reached via HWT_RELEASE_BASE_URL=file://... Asserts external behavior
//! only: the script's exit status and what ends up (or not) at
//! target/release/herdr-window-title.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BINARY_REL_PATH: &str = "target/release/herdr-window-title";
const FAKE_BINARY: &str = "#!/bin/sh\necho fake herdr-window-title\n";
const ALL_TARGETS: [&str; 4] = [
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
];

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The target triple the script must resolve on this test host.
fn host_target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        other => panic!("unsupported test host: {other:?}"),
    }
}

/// Copy the real manifest and script into a fresh plugin-root directory,
/// exactly the layout `herdr plugin install` clones before running builds.
fn make_plugin_root(root: &Path) {
    fs::create_dir_all(root.join("scripts")).expect("create plugin root");
    fs::copy(
        manifest_dir().join("herdr-plugin.toml"),
        root.join("herdr-plugin.toml"),
    )
    .expect("copy manifest");
    fs::copy(
        manifest_dir().join("scripts/fetch-release.sh"),
        root.join("scripts/fetch-release.sh"),
    )
    .expect("copy script");
}

/// The version the script will read from the copied herdr-plugin.toml.
fn manifest_version(plugin_root: &Path) -> String {
    let manifest =
        fs::read_to_string(plugin_root.join("herdr-plugin.toml")).expect("read manifest");
    for line in manifest.lines() {
        if let Some(rest) = line.strip_prefix("version") {
            if let Some(value) = rest.trim_start().strip_prefix('=') {
                return value.trim().trim_matches('"').to_string();
            }
        }
    }
    panic!("no version key in herdr-plugin.toml");
}

/// Build a release tarball whose layout is target/release/herdr-window-title,
/// as the release workflow packages it. Returns the tarball path.
fn stage_tarball(release_dir: &Path, version: &str, target: &str) -> PathBuf {
    let stage = release_dir.join("stage");
    let bin_dir = stage.join("target/release");
    fs::create_dir_all(&bin_dir).expect("create staging dirs");
    let bin = bin_dir.join("herdr-window-title");
    fs::write(&bin, FAKE_BINARY).expect("write fake binary");
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).expect("chmod fake binary");

    let archive = release_dir.join(format!("herdr-window-title-v{version}-{target}.tar.gz"));
    let status = Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&stage)
        .arg(BINARY_REL_PATH)
        .status()
        .expect("tar runs");
    assert!(status.success(), "tar failed to package the fake release");
    fs::remove_dir_all(&stage).expect("drop staging dir");
    archive
}

/// Write a SHA256SUMS covering all four release tarballs, with a real hash
/// for the host target's tarball and placeholder hashes for the other three.
fn write_sums(release_dir: &Path, archive: &Path, version: &str) {
    let real_name = archive
        .file_name()
        .and_then(|n| n.to_str())
        .expect("archive name");
    let real_hash = sha256_hex(archive);
    let mut sums = String::new();
    for target in ALL_TARGETS {
        let name = format!("herdr-window-title-v{version}-{target}.tar.gz");
        if name == real_name {
            sums.push_str(&format!("{real_hash}  {name}\n"));
        } else {
            sums.push_str(&format!("{}  {name}\n", "0".repeat(64)));
        }
    }
    fs::write(release_dir.join("SHA256SUMS"), sums).expect("write SHA256SUMS");
}

fn sha256_hex(path: &Path) -> String {
    let output = Command::new("sha256sum")
        .arg(path)
        .output()
        .or_else(|_| Command::new("shasum").args(["-a", "256"]).arg(path).output())
        .expect("a sha256 tool is available");
    assert!(output.status.success(), "sha256 tool failed");
    String::from_utf8(output.stdout)
        .expect("utf8 digest")
        .split_whitespace()
        .next()
        .expect("digest field")
        .to_string()
}

fn run_fetch(plugin_root: &Path, release_dir: &Path) -> Output {
    Command::new("sh")
        .arg("scripts/fetch-release.sh")
        .current_dir(plugin_root)
        .env("HWT_RELEASE_BASE_URL", format!("file://{}", release_dir.display()))
        .env_remove("GITHUB_TOKEN")
        .output()
        .expect("script runs")
}

/// Fresh plugin root + fake release dir with a valid tarball and SHA256SUMS.
fn set_up(tmp: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let plugin_root = tmp.join("plugin");
    make_plugin_root(&plugin_root);
    let version = manifest_version(&plugin_root);
    let release_dir = tmp.join("release");
    fs::create_dir_all(&release_dir).expect("create release dir");
    let archive = stage_tarball(&release_dir, &version, host_target());
    write_sums(&release_dir, &archive, &version);
    (plugin_root, release_dir, archive)
}

#[test]
fn happy_path_installs_executable_binary_at_expected_path() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let (plugin_root, release_dir, _archive) = set_up(tmp.path());

    let output = run_fetch(&plugin_root, &release_dir);

    assert!(
        output.status.success(),
        "script failed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let installed = plugin_root.join(BINARY_REL_PATH);
    assert!(installed.is_file(), "no binary at {BINARY_REL_PATH}");
    let mode = fs::metadata(&installed)
        .expect("stat installed binary")
        .permissions()
        .mode();
    assert!(mode & 0o111 != 0, "installed binary is not executable (mode {mode:o})");
    assert_eq!(
        fs::read_to_string(&installed).expect("read installed binary"),
        FAKE_BINARY,
        "installed binary is not the staged release binary"
    );
}

#[test]
fn tampered_tarball_fails_with_checksum_error_and_leaves_no_binary() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let (plugin_root, release_dir, archive) = set_up(tmp.path());
    // Tamper after SHA256SUMS was generated.
    fs::OpenOptions::new()
        .append(true)
        .open(&archive)
        .expect("open tarball for tampering")
        .write_all(b"tampered")
        .expect("append tamper bytes");

    let output = run_fetch(&plugin_root, &release_dir);

    assert!(
        !output.status.success(),
        "script must exit nonzero on a tampered tarball"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("checksum mismatch"),
        "expected a clear checksum error, got: {stderr}"
    );
    assert!(
        !plugin_root.join(BINARY_REL_PATH).exists(),
        "tampered download must not leave a binary at {BINARY_REL_PATH}"
    );
    assert!(
        !plugin_root.join("target").exists(),
        "tampered download must not leave partial output under target/"
    );
}

#[test]
fn missing_release_asset_fails_hard_and_leaves_no_binary() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let plugin_root = tmp.path().join("plugin");
    make_plugin_root(&plugin_root);
    let release_dir = tmp.path().join("release");
    fs::create_dir_all(&release_dir).expect("create empty release dir");

    let output = run_fetch(&plugin_root, &release_dir);

    assert!(
        !output.status.success(),
        "script must exit nonzero when the release asset is missing"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to download"),
        "expected a clear download error, got: {stderr}"
    );
    assert!(
        !plugin_root.join(BINARY_REL_PATH).exists(),
        "failed download must not leave a binary at {BINARY_REL_PATH}"
    );
}
