//! Directory batch ordering and publication contracts.

#![cfg(feature = "cli")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("qrencodes-directory-{name}-{}-{stamp}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(input: &Path, output: &Path, format: &str, parallel: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_qrencodes"));
    command
        .args(["--batch"])
        .arg(input)
        .args(["--batch-format", format, "--batch-pack", "directory", "-f", "svg", "-o"])
        .arg(output);
    if parallel {
        command.arg("--parallel");
    }
    command.output().unwrap()
}

fn record_bytes(payloads: &[String], format: &str) -> Vec<u8> {
    match format {
        "json" => serde_json::to_vec(payloads).unwrap(),
        "jsonl" => {
            payloads.iter().map(|text| serde_json::to_string(text).unwrap()).collect::<Vec<_>>().join("\n").into_bytes()
        }
        "csv" => payloads
            .iter()
            .map(|text| format!("\"{}\"", text.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join("\n")
            .into_bytes(),
        _ => payloads.join("\n").into_bytes(),
    }
}

fn expected_svg(text: &str) -> Vec<u8> {
    qrcode_rs::QrCode::new(text)
        .unwrap()
        .render::<qrcode_rs::render::svg::Color>()
        .dark_color(qrcode_rs::render::svg::Color("#000000"))
        .light_color(qrcode_rs::render::svg::Color("#ffffff"))
        .build()
        .into_bytes()
}

#[test]
fn parallel_directory_payloads_and_names_stay_ordered_for_large_batches() {
    let directory = TestDirectory::new("ordered");
    let input = directory.0.join("records");
    let payloads = (0..131).map(|index| format!("payload-{index:04}")).collect::<Vec<_>>();
    let expected = payloads.iter().map(|text| expected_svg(text)).collect::<Vec<_>>();
    for format in ["lines", "csv", "jsonl", "json"] {
        fs::write(&input, record_bytes(&payloads, format)).unwrap();
        let output = directory.0.join(format);
        let result = run(&input, &output, format, true);
        assert!(result.status.success(), "{format}: {}", String::from_utf8_lossy(&result.stderr));
        assert_eq!(fs::read_dir(&output).unwrap().count(), expected.len());
        for (index, expected) in expected.iter().enumerate() {
            let name = format!("qr-{:04}.svg", index + 1);
            assert_eq!(fs::read(output.join(name)).unwrap(), *expected);
        }
    }
}

#[test]
fn complete_input_validation_still_precedes_parallel_render_errors() {
    let directory = TestDirectory::new("parse-priority");
    let input = directory.0.join("records");
    let oversized = "x".repeat(4_000);
    let mut invalid_lines = format!("{oversized}\n").into_bytes();
    invalid_lines.extend_from_slice(b"\xff\n");
    for (format, bytes) in [
        ("lines", invalid_lines),
        ("csv", format!("{oversized}\nbad\"quote\"\n").into_bytes()),
        ("jsonl", format!("\"{oversized}\"\n{{\"text\":42}}\n").into_bytes()),
        ("json", format!("[\"{oversized}\",42]").into_bytes()),
    ] {
        fs::write(&input, bytes).unwrap();
        for parallel in [true, false].into_iter().filter(|parallel| *parallel || format == "json") {
            let output = directory.0.join(format!("{format}-{parallel}"));
            let result = run(&input, &output, format, parallel);
            assert_eq!(result.status.code(), Some(1));
            assert!(!String::from_utf8_lossy(&result.stderr).contains("data too long"));
            assert!(!output.exists(), "published output before input validation: {format}, {parallel}");
        }
    }
}

#[test]
fn late_parallel_render_errors_publish_no_files_or_replacements() {
    let directory = TestDirectory::new("late-render-error");
    let input = directory.0.join("records");
    let mut payloads = (0..130).map(|index| format!("payload-{index:04}")).collect::<Vec<_>>();
    payloads.push("x".repeat(4_000));
    for format in ["lines", "csv", "jsonl", "json"] {
        fs::write(&input, record_bytes(&payloads, format)).unwrap();
        for existing in [false, true] {
            let output = directory.0.join(format!("{format}-{existing}"));
            let first = output.join("qr-0001.svg");
            if existing {
                fs::create_dir(&output).unwrap();
                fs::write(&first, b"previous output").unwrap();
            }
            let result = run(&input, &output, format, true);
            assert_eq!(result.status.code(), Some(1));
            assert!(String::from_utf8_lossy(&result.stderr).contains("data too long"));
            if existing {
                assert_eq!(fs::read(&first).unwrap(), b"previous output");
                assert_eq!(fs::read_dir(&output).unwrap().count(), 1);
            } else {
                assert!(!output.exists());
            }
        }
    }
}

#[test]
fn sequential_json_retains_published_prefix_on_a_later_render_error() {
    let directory = TestDirectory::new("sequential-json");
    let input = directory.0.join("records.json");
    let output = directory.0.join("output");
    fs::create_dir(&output).unwrap();
    let third = output.join("qr-0003.svg");
    fs::write(&third, b"previous third output").unwrap();
    fs::write(&input, record_bytes(&["alpha".to_owned(), "beta".to_owned(), "x".repeat(4_000)], "json")).unwrap();
    let result = run(&input, &output, "json", false);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("data too long"));
    assert_eq!(fs::read(output.join("qr-0001.svg")).unwrap(), expected_svg("alpha"));
    assert_eq!(fs::read(output.join("qr-0002.svg")).unwrap(), expected_svg("beta"));
    assert_eq!(fs::read(third).unwrap(), b"previous third output");
    assert_eq!(fs::read_dir(&output).unwrap().count(), 3);
}

#[test]
fn parallel_directory_write_errors_keep_the_already_published_prefix() {
    let directory = TestDirectory::new("write-error");
    let input = directory.0.join("records");
    let output = directory.0.join("output");
    fs::create_dir(&output).unwrap();
    let second = output.join("qr-0002.svg");
    fs::write(&second, b"previous read-only output").unwrap();
    let original = fs::metadata(&second).unwrap().permissions();
    let mut read_only = original.clone();
    read_only.set_readonly(true);
    fs::set_permissions(&second, read_only).unwrap();
    fs::write(&input, "alpha\nbeta\ngamma\n").unwrap();
    let result = run(&input, &output, "lines", true);
    fs::set_permissions(&second, original).unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(fs::read(output.join("qr-0001.svg")).unwrap(), expected_svg("alpha"));
    assert_eq!(fs::read(&second).unwrap(), b"previous read-only output");
    assert!(!output.join("qr-0003.svg").exists());
    assert_eq!(fs::read_dir(&output).unwrap().count(), 2);
}
