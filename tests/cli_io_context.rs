//! File path diagnostics at concrete CLI I/O boundaries.

#![cfg(feature = "cli")]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_TEST_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("qrencodes-io-context-{}-{stamp}-{sequence}", std::process::id()));
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
fn missing_batch_inputs_report_the_source_path_for_line_and_json_readers() {
    let directory = TestDirectory::new();
    let missing = directory.0.join("missing-records");
    for format in ["lines", "json"] {
        let output = directory.0.join(format);
        let result = Command::new(env!("CARGO_BIN_EXE_qrencodes"))
            .args(["--batch"])
            .arg(&missing)
            .args(["--batch-format", format, "-f", "svg", "-o"])
            .arg(&output)
            .output()
            .unwrap();
        let error = String::from_utf8(result.stderr).unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(error.contains("open batch input"), "{error}");
        assert!(error.contains(&missing.display().to_string()), "{error}");
        assert!(!output.exists());
    }
}

#[test]
fn missing_validation_images_report_the_image_path() {
    let directory = TestDirectory::new();
    let missing = directory.0.join("missing-image.png");
    let result = Command::new(env!("CARGO_BIN_EXE_qrencodes")).arg("validate").arg(&missing).output().unwrap();
    let error = String::from_utf8(result.stderr).unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(error.contains("read validation image"), "{error}");
    assert!(error.contains(&missing.display().to_string()), "{error}");
}

#[test]
fn single_file_output_failures_report_the_destination_path() {
    let directory = TestDirectory::new();
    let output = directory.0.join("missing-parent").join("output.svg");
    let result =
        Command::new(env!("CARGO_BIN_EXE_qrencodes")).args(["alpha", "-f", "svg", "-o"]).arg(&output).output().unwrap();
    let error = String::from_utf8(result.stderr).unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(error.contains("write output file"), "{error}");
    assert!(error.contains(&output.display().to_string()), "{error}");
    assert!(!output.exists());
}

#[test]
fn failed_zip_creation_reports_the_archive_path_and_preserves_existing_input() {
    let directory = TestDirectory::new();
    let input = directory.0.join("records");
    let output = directory.0.join("missing-parent").join("output.zip");
    fs::write(&input, b"alpha\n").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_qrencodes"))
        .args(["--batch"])
        .arg(&input)
        .args(["--batch-pack", "zip", "-f", "svg", "-o"])
        .arg(&output)
        .output()
        .unwrap();
    let error = String::from_utf8(result.stderr).unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(error.contains("create ZIP output"), "{error}");
    assert!(error.contains(&output.display().to_string()), "{error}");
    assert_eq!(fs::read(&input).unwrap(), b"alpha\n");
    assert!(!output.exists());
}
