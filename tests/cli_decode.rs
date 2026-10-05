//! Image command integration for Normal, Micro, binary, and Structured Append QR.

#![cfg(feature = "cli")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use qrcode_rs::batch::{BatchEntry, BatchGridOptions, BatchOutput};
use qrcode_rs::{Color, EcLevel, ModuleStorage, QrCode, QrTemplate, Version};

static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("qrencodes-decode-{}-{stamp}-{sequence}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn binary() -> Command {
    // Exercise the same contracts against a rebuilt published-package binary.
    let path = std::env::var_os("QRCODE_TEST_CLI_BIN").unwrap_or_else(|| env!("CARGO_BIN_EXE_qrencodes").into());
    Command::new(path)
}

fn save_codes(path: &Path, codes: Vec<QrCode>) {
    let batch = BatchOutput::from_entries(codes.into_iter().map(|code| BatchEntry::new("", code)));
    let bytes = batch
        .to_png_grid_with(BatchGridOptions::default().columns(2), &QrTemplate::minimal().with_module_size(8, 8))
        .unwrap();
    fs::write(path, bytes).unwrap();
}

fn save_code(path: &Path, code: &QrCode) {
    code.render::<qrcode_rs::render::image::Luma<u8>>().module_dimensions(8, 8).build().save(path).unwrap();
}

#[test]
fn decode_json_and_validate_support_normal_multicode_and_micro_symbols() {
    let directory = TestDirectory::new();
    let path = directory.0.join("mixed.png");
    save_codes(
        &path,
        vec![
            QrCode::new("alpha").unwrap(),
            QrCode::new("beta").unwrap(),
            QrCode::with_version(b"123", Version::Micro(1), EcLevel::L).unwrap(),
        ],
    );
    let decoded = binary().arg("decode").arg(&path).args(["--format", "json"]).output().unwrap();
    assert!(decoded.status.success(), "{}", String::from_utf8_lossy(&decoded.stderr));
    let value: serde_json::Value = serde_json::from_slice(&decoded.stdout).unwrap();
    let symbols = value["symbols"].as_array().unwrap();
    assert_eq!(symbols.len(), 3);
    let texts = symbols.iter().map(|symbol| symbol["text"].as_str().unwrap()).collect::<Vec<_>>();
    for text in ["alpha", "beta", "123"] {
        assert!(texts.contains(&text));
    }
    assert!(symbols.iter().any(|symbol| symbol["version"]["kind"] == "micro"));
    assert!(symbols.iter().all(|symbol| symbol["source_image"] == path.display().to_string()));
    let validated = binary().arg("validate").arg(&path).args(["--expect", "123"]).output().unwrap();
    assert!(validated.status.success(), "{}", String::from_utf8_lossy(&validated.stderr));
    assert_eq!(String::from_utf8(validated.stdout).unwrap(), "valid: decoded 3 QR code(s)\n");
}

#[test]
fn decode_raw_preserves_binary_bytes_and_text_failure_preserves_existing_file() {
    let directory = TestDirectory::new();
    let path = directory.0.join("binary.png");
    let output = directory.0.join("output");
    let bytes = [0, 255, 128, 16, 0];
    save_code(&path, &QrCode::builder(bytes).encoding_mode(qrcode_rs::Mode::Byte).build().unwrap());
    let raw = binary().arg("decode").arg(&path).args(["--format", "raw"]).output().unwrap();
    assert!(raw.status.success(), "{}", String::from_utf8_lossy(&raw.stderr));
    assert_eq!(raw.stdout, bytes);
    fs::write(&output, b"previous output").unwrap();
    let text = binary().arg("decode").arg(&path).args(["--format", "text", "--output"]).arg(&output).output().unwrap();
    assert_eq!(text.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&text.stderr).contains("valid UTF-8"));
    assert_eq!(fs::read(&output).unwrap(), b"previous output");
    let json = binary().arg("decode").arg(&path).args(["--format", "json"]).output().unwrap();
    assert!(json.status.success(), "{}", String::from_utf8_lossy(&json.stderr));
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(value["symbols"][0]["data"], serde_json::json!(bytes));
    assert!(value["symbols"][0]["text"].is_null());
}

#[test]
fn raw_multiple_payloads_are_rejected_before_publication() {
    let directory = TestDirectory::new();
    let path = directory.0.join("multiple.png");
    let output = directory.0.join("output");
    save_codes(&path, vec![QrCode::new("alpha").unwrap(), QrCode::new("beta").unwrap()]);
    fs::write(&output, b"previous").unwrap();
    let result = binary().arg("decode").arg(&path).args(["--format", "raw", "--output"]).arg(&output).output().unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("exactly one logical payload"));
    assert_eq!(fs::read(&output).unwrap(), b"previous");
}

#[test]
fn explicit_structured_append_assembly_keeps_fragments_and_rejects_missing_or_duplicates() {
    let directory = TestDirectory::new();
    let payload = b"structured append payload";
    let codes = QrCode::structured_append(payload, 2, EcLevel::M).unwrap();
    let first = directory.0.join("first.png");
    let second = directory.0.join("second.png");
    save_code(&first, &codes[0]);
    save_code(&second, &codes[1]);
    let result =
        binary().arg("decode").arg(&second).arg(&first).args(["--assemble", "--format", "raw"]).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert_eq!(result.stdout, payload);
    let json =
        binary().arg("decode").arg(&second).arg(&first).args(["--assemble", "--format", "json"]).output().unwrap();
    assert!(json.status.success(), "{}", String::from_utf8_lossy(&json.stderr));
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(value["symbols"].as_array().unwrap().len(), 2);
    assert_eq!(value["assemblies"].as_array().unwrap().len(), 1);
    assert_eq!(value["assemblies"][0]["data"], serde_json::json!(payload));
    assert_eq!(value["symbols"][0]["structured_append"]["position"], 2);
    let output = directory.0.join("output");
    fs::write(&output, b"previous").unwrap();
    for inputs in [vec![&first], vec![&first, &first, &second]] {
        let bad = binary()
            .arg("decode")
            .args(inputs)
            .args(["--assemble", "--allow-partial", "--format", "raw", "--output"])
            .arg(&output)
            .output()
            .unwrap();
        assert_eq!(bad.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&bad.stderr).contains("Structured Append group"));
        assert_eq!(fs::read(&output).unwrap(), b"previous");
    }
}

#[test]
fn independent_structured_append_groups_are_assembled_separately() {
    let directory = TestDirectory::new();
    let a = b"alpha sequence";
    let b = b"beta sequence";
    assert_ne!(a.iter().fold(0u8, |x, b| x ^ b), b.iter().fold(0u8, |x, b| x ^ b));
    let mut paths = Vec::new();
    for (group, data) in [a.as_slice(), b.as_slice()].into_iter().enumerate() {
        for (part, code) in QrCode::structured_append(data, 2, EcLevel::M).unwrap().iter().enumerate() {
            let path = directory.0.join(format!("group-{group}-part-{part}.png"));
            save_code(&path, code);
            paths.push(path);
        }
    }
    let result = binary().arg("decode").args(&paths).args(["--assemble", "--format", "json"]).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["symbols"].as_array().unwrap().len(), 4);
    assert_eq!(value["assemblies"].as_array().unwrap().len(), 2);
    assert_eq!(value["assemblies"][0]["data"], serde_json::json!(a));
    assert_eq!(value["assemblies"][1]["data"], serde_json::json!(b));
}

#[test]
fn decode_limits_and_same_path_outputs_do_not_truncate_unread_images() {
    let directory = TestDirectory::new();
    let path = directory.0.join("input.png");
    save_code(&path, &QrCode::new("alpha").unwrap());
    let original = fs::read(&path).unwrap();
    let bad = binary()
        .arg("decode")
        .arg(&path)
        .args(["--max-pixels", "1", "--format", "raw", "--output"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(1));
    assert_eq!(fs::read(&path).unwrap(), original);
    let good = binary().arg("decode").arg(&path).args(["--format", "raw", "--output"]).arg(&path).output().unwrap();
    assert!(good.status.success(), "{}", String::from_utf8_lossy(&good.stderr));
    assert_eq!(fs::read(&path).unwrap(), b"alpha");
}

#[test]
fn decode_inverted_images_uses_the_explicit_polarity_option() {
    let directory = TestDirectory::new();
    let path = directory.0.join("inverted.png");
    let mut image = QrCode::new("alpha").unwrap().render::<qrcode_rs::render::image::Luma<u8>>().build();
    for pixel in image.pixels_mut() {
        pixel.0[0] = 255 - pixel.0[0];
    }
    image.save(&path).unwrap();
    let result = binary().arg("decode").arg(&path).args(["--invert", "--format", "raw"]).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert_eq!(result.stdout, b"alpha");
}

#[test]
fn decode_generation_input_conflicts_use_the_decode_command_name() {
    for (args, expected) in [
        (vec!["alpha", "decode", "missing.png"], "TEXT cannot be used together with decode"),
        (vec!["--batch", "missing-records", "decode", "missing.png"], "--batch cannot be used together with decode"),
        (
            vec!["alpha", "--batch", "missing-records", "decode", "missing.png"],
            "--batch cannot be used together with TEXT",
        ),
    ] {
        let result = binary().args(args).output().unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert_eq!(String::from_utf8(result.stderr).unwrap(), format!("error: {expected}\n"));
    }
}

#[test]
fn strict_and_partial_candidate_failures_have_distinct_publication_policies() {
    let directory = TestDirectory::new();
    let mut damaged = QrCode::new("damaged").unwrap();
    for y in 9..damaged.width() {
        for x in 9..damaged.width() {
            damaged.set(x, y, if (x * 37 + y * 13) % 3 == 0 { Color::Dark } else { Color::Light });
        }
    }
    let path = directory.0.join("partial.png");
    save_codes(&path, vec![QrCode::new("alpha").unwrap(), damaged]);
    let output = directory.0.join("output");
    fs::write(&output, b"previous").unwrap();
    let strict =
        binary().arg("decode").arg(&path).args(["--format", "json", "--output"]).arg(&output).output().unwrap();
    assert_eq!(strict.status.code(), Some(1));
    assert_eq!(fs::read(&output).unwrap(), b"previous");
    let partial = binary()
        .arg("decode")
        .arg(&path)
        .args(["--allow-partial", "--format", "json", "--output"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(partial.status.success(), "{}", String::from_utf8_lossy(&partial.stderr));
    assert!(String::from_utf8_lossy(&partial.stderr).contains("warning:"));
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(value["symbols"].as_array().unwrap().len(), 1);
    assert!(!value["errors"].as_array().unwrap().is_empty());
}
