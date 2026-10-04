//! Stdout pipe termination and error-origin behavior.

#![cfg(feature = "cli")]

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn closed_stdout(command: &mut Command) -> Output {
    let mut child = command.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    drop(child.stdout.take());
    child.wait_with_output().unwrap()
}

fn closed_stdout_before_input(command: &mut Command, input: &[u8]) -> Output {
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    drop(child.stdout.take());
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_qrencodes"))
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("qrencodes-stdout-{name}-{}-{stamp}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn generated_stdout_closed_by_the_reader_exits_successfully_without_errors() {
    for explicit_dash in [false, true] {
        let mut command = binary();
        command.args(["-f", "string"]);
        if explicit_dash {
            command.args(["-o", "-"]);
        }
        let result = closed_stdout_before_input(&mut command, b"alpha");
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        assert!(result.stderr.is_empty());
    }
}

#[test]
fn validation_stdout_closed_by_the_reader_does_not_panic() {
    let directory = TestDirectory::new("validate");
    let image = directory.0.join("input.png");
    let generated = binary().args(["alpha", "-f", "png", "-o"]).arg(&image).output().unwrap();
    assert!(generated.status.success(), "{}", String::from_utf8_lossy(&generated.stderr));
    let result = closed_stdout(binary().arg("validate").arg(&image).arg("--print-payload"));
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert!(result.stderr.is_empty());
}

#[test]
fn a_closed_stdout_does_not_silence_render_errors() {
    let result = closed_stdout(binary().arg("x".repeat(4_000)).args(["-f", "string"]));
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("data too long"));
}

#[test]
fn a_closed_stdout_does_not_silence_regular_file_output_errors() {
    let directory = TestDirectory::new("file-error");
    let output = directory.0.join("missing-parent").join("output.svg");
    let result = closed_stdout(binary().args(["alpha", "-f", "svg", "-o"]).arg(&output));
    assert_eq!(result.status.code(), Some(1));
    assert!(!result.stderr.is_empty());
    assert!(!output.exists());
}

#[test]
fn a_closed_stdout_does_not_silence_validation_expectation_errors() {
    let directory = TestDirectory::new("expect-error");
    let image = directory.0.join("input.png");
    let generated = binary().args(["alpha", "-f", "png", "-o"]).arg(&image).output().unwrap();
    assert!(generated.status.success(), "{}", String::from_utf8_lossy(&generated.stderr));
    let result = closed_stdout(binary().arg("validate").arg(&image).args(["--expect", "different"]));
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("none matched"));
}
