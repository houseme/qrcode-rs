//! Smoke tests for the `qrencodes` CLI binary.
//!
//! These only run when the `cli` feature is enabled (which is what builds the
//! binary under test). Run with `cargo test --all-features`.

#![cfg(feature = "cli")]

use std::io::Write;
use std::process::{Command, Stdio};
use std::time;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_qrencodes"))
}

#[test]
fn help_exits_zero() {
    let out = bin().arg("--help").output().expect("spawn qrencodes");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("Usage:"));
}

#[test]
fn version_flag_exits_zero() {
    let out = bin().arg("--version").output().expect("spawn qrencodes");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("qrencodes"));
}

#[test]
fn string_format_to_stdout() {
    let out = bin().args(["-f", "string", "--no-quiet-zone", "hi"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains('#'));
}

#[test]
fn svg_to_file() {
    let path = std::env::temp_dir().join(format!("qrencodes_cli_{}.svg", line!()));
    let out = bin().args(["-f", "svg", "-o"]).arg(&path).arg("hello").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("<svg"));
}

#[test]
fn png_to_file_is_valid_png() {
    let path = std::env::temp_dir().join(format!("qrencodes_cli_{}.png", line!()));
    let out = bin().args(["-f", "png", "-o"]).arg(&path).arg("hello").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let bytes = std::fs::read(&path).unwrap();
    // PNG signature: 89 50 4E 47 0D 0A 1A 0A
    assert_eq!(&bytes[..8], &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
}

#[test]
fn stdin_input() {
    let mut child =
        bin().args(["-f", "string", "--no-quiet-zone"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(b"piped").unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains('#'));
}

#[test]
fn invalid_version_errors() {
    let out = bin().args(["-v", "99", "hi"]).output().unwrap();
    assert!(!out.status.success(), "expected non-zero exit for -v 99");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("1 and 40") || stderr.contains("QR version"));
}

#[test]
fn batch_writes_multiple_files() {
    let stamp = time::SystemTime::now().duration_since(time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("qrencodes_cli_batch_{stamp}"));
    let list = std::env::temp_dir().join(format!("qrencodes_cli_list_{stamp}.txt"));
    std::fs::write(&list, "aaa\nbbb\n").unwrap();

    let out = bin().args(["--batch"]).arg(&list).args(["-f", "svg", "-o"]).arg(&dir).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.join("qr-0001.svg").exists(), "missing qr-0001.svg");
    assert!(dir.join("qr-0002.svg").exists(), "missing qr-0002.svg");
}
