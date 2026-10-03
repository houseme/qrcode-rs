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

fn temporary_directory(name: &str) -> std::path::PathBuf {
    let stamp = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("qrencodes_cli_{name}_{}_{stamp}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    dir
}

// Decode stored local records to verify the archive contains complete rendered
// payloads, rather than checking only its signature or embedded file names.
fn stored_zip_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut offset = 0;
    let mut entries = Vec::new();
    while bytes.get(offset..offset + 4) == Some(b"PK\x03\x04") {
        let compression = u16::from_le_bytes(bytes[offset + 8..offset + 10].try_into().unwrap());
        assert_eq!(compression, 0);
        let size = u32::from_le_bytes(bytes[offset + 18..offset + 22].try_into().unwrap()) as usize;
        let name_len = u16::from_le_bytes(bytes[offset + 26..offset + 28].try_into().unwrap()) as usize;
        let extra_len = u16::from_le_bytes(bytes[offset + 28..offset + 30].try_into().unwrap()) as usize;
        let name_start = offset + 30;
        let payload_start = name_start + name_len + extra_len;
        let name = String::from_utf8(bytes[name_start..name_start + name_len].to_vec()).unwrap();
        let payload = bytes[payload_start..payload_start + size].to_vec();
        entries.push((name, payload));
        offset = payload_start + size;
    }
    assert_eq!(&bytes[offset..offset + 4], b"PK\x01\x02");
    assert_eq!(&bytes[bytes.len() - 22..bytes.len() - 18], b"PK\x05\x06");
    entries
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
    std::fs::remove_file(list).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn csv_batch_writes_selected_column_in_parallel() {
    let stamp = time::SystemTime::now().duration_since(time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("qrencodes_cli_csv_batch_{stamp}"));
    let list = std::env::temp_dir().join(format!("qrencodes_cli_csv_list_{stamp}.csv"));
    std::fs::write(&list, "1,alpha\n2,\"beta, payload\"\n").unwrap();

    let out = bin()
        .args(["--batch"])
        .arg(&list)
        .args(["--batch-format", "csv", "--batch-column", "2", "--parallel", "-f", "svg", "-o"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.join("qr-0001.svg").exists(), "missing qr-0001.svg");
    assert!(dir.join("qr-0002.svg").exists(), "missing qr-0002.svg");
    std::fs::remove_file(list).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn csv_multiline_records_match_rendered_payloads_in_directory_and_zip_modes() {
    let dir = temporary_directory("csv_multiline");
    let input = dir.join("records.csv");
    std::fs::write(&input, "\n1,\"alpha\nbeta\"\n2,\"say \"\"hello\"\"\r\nnext\"\r\n3,\"\"\r\n").unwrap();
    let expected = ["alpha\nbeta", "say \"hello\"\r\nnext"].map(|text| {
        qrcode_rs::QrCode::new(text)
            .unwrap()
            .render::<qrcode_rs::render::svg::Color>()
            .dark_color(qrcode_rs::render::svg::Color("#000000"))
            .light_color(qrcode_rs::render::svg::Color("#ffffff"))
            .build()
            .into_bytes()
    });
    for pack in ["directory", "zip"] {
        for parallel in [false, true] {
            let output = dir.join(format!("{pack}-{parallel}"));
            let mut command = bin();
            command
                .args(["--batch"])
                .arg(&input)
                .args(["--batch-format", "csv", "--batch-column", "2", "--batch-pack", pack, "-f", "svg", "-o"])
                .arg(&output);
            if parallel {
                command.arg("--parallel");
            }
            let result = command.output().unwrap();
            assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
            let actual = if pack == "zip" {
                stored_zip_entries(&std::fs::read(&output).unwrap())
            } else {
                assert_eq!(std::fs::read_dir(&output).unwrap().count(), 2);
                (1..=2)
                    .map(|index| {
                        let name = format!("qr-{index:04}.svg");
                        let bytes = std::fs::read(output.join(&name)).unwrap();
                        (name, bytes)
                    })
                    .collect()
            };
            assert_eq!(actual.len(), expected.len());
            for (index, ((name, actual), expected)) in actual.iter().zip(&expected).enumerate() {
                assert_eq!(name, &format!("qr-{:04}.svg", index + 1));
                assert_eq!(actual, expected);
            }
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn csv_invalid_quotes_and_unclosed_multiline_records_preserve_zip_output() {
    let dir = temporary_directory("csv_errors");
    let input = dir.join("records.csv");
    let output = dir.join("output.zip");
    for record in ["2,bad\"quote\"\n", "2,\"closed\"extra\n", "2,\"unclosed\ncontinued\n"] {
        std::fs::write(&input, format!("1,valid\n{record}")).unwrap();
        std::fs::write(&output, b"existing output").unwrap();
        for parallel in [false, true] {
            let mut command = bin();
            command
                .args(["--batch"])
                .arg(&input)
                .args(["--batch-format", "csv", "--batch-column", "2", "--batch-pack", "zip", "-f", "svg", "-o"])
                .arg(&output);
            if parallel {
                command.arg("--parallel");
            }
            let result = command.output().unwrap();
            assert!(!result.status.success());
            assert!(String::from_utf8_lossy(&result.stderr).contains("batch line 2:"));
            assert_eq!(std::fs::read(&output).unwrap(), b"existing output");
            assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn jsonl_batch_writes_named_key() {
    let stamp = time::SystemTime::now().duration_since(time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("qrencodes_cli_jsonl_batch_{stamp}"));
    let list = std::env::temp_dir().join(format!("qrencodes_cli_jsonl_list_{stamp}.jsonl"));
    std::fs::write(&list, "{\"payload\":\"alpha\"}\n{\"payload\":\"beta\"}\n").unwrap();

    let out = bin()
        .args(["--batch"])
        .arg(&list)
        .args(["--batch-format", "jsonl", "--batch-key", "payload", "-f", "svg", "-o"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.join("qr-0001.svg").exists(), "missing qr-0001.svg");
    assert!(dir.join("qr-0002.svg").exists(), "missing qr-0002.svg");
    std::fs::remove_file(list).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn json_batch_writes_array_payloads() {
    let stamp = time::SystemTime::now().duration_since(time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("qrencodes_cli_json_batch_{stamp}"));
    let list = std::env::temp_dir().join(format!("qrencodes_cli_json_list_{stamp}.json"));
    std::fs::write(&list, r#"[{"payload":"alpha"},"beta",{"payload":""}]"#).unwrap();

    let out = bin()
        .args(["--batch"])
        .arg(&list)
        .args(["--batch-format", "json", "--batch-key", "payload", "-f", "svg", "-o"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.join("qr-0001.svg").exists(), "missing qr-0001.svg");
    assert!(dir.join("qr-0002.svg").exists(), "missing qr-0002.svg");

    std::fs::remove_file(list).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn zip_batch_writes_archive_entries() {
    let stamp = time::SystemTime::now().duration_since(time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let archive = std::env::temp_dir().join(format!("qrencodes_cli_zip_batch_{stamp}.zip"));
    let list = std::env::temp_dir().join(format!("qrencodes_cli_zip_list_{stamp}.txt"));
    std::fs::write(&list, "alpha\nbeta\n").unwrap();

    let out = bin()
        .args(["--batch"])
        .arg(&list)
        .args(["--batch-pack", "zip", "-f", "svg", "-o"])
        .arg(&archive)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let bytes = std::fs::read(&archive).unwrap();
    assert!(bytes.starts_with(b"PK\x03\x04"));
    assert!(bytes.windows(b"qr-0001.svg".len()).any(|window| window == b"qr-0001.svg"));
    assert!(bytes.windows(b"qr-0002.svg".len()).any(|window| window == b"qr-0002.svg"));

    std::fs::remove_file(list).unwrap();
    std::fs::remove_file(archive).unwrap();
}

#[test]
fn zip_batch_can_replace_its_input_after_reading_all_records() {
    let dir = temporary_directory("zip_same_path");
    let path = dir.join("records.txt");
    std::fs::write(&path, "alpha\nbeta\n").unwrap();

    let out = bin()
        .args(["--batch"])
        .arg(&path)
        .args(["--batch-pack", "zip", "-f", "svg", "-o"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let entries = stored_zip_entries(&std::fs::read(&path).unwrap());
    assert_eq!(entries.len(), 2);
    for ((name, payload), (expected_name, text)) in
        entries.iter().zip([("qr-0001.svg", "alpha"), ("qr-0002.svg", "beta")])
    {
        let expected = qrcode_rs::QrCode::new(text)
            .unwrap()
            .render::<qrcode_rs::render::svg::Color>()
            .dark_color(qrcode_rs::render::svg::Color("#000000"))
            .light_color(qrcode_rs::render::svg::Color("#ffffff"))
            .build();
        assert_eq!(name, expected_name);
        assert_eq!(payload, expected.as_bytes());
    }
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "temporary ZIP file was not removed");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn zip_batch_preserves_existing_output_on_input_and_render_errors() {
    let dir = temporary_directory("zip_failure");
    let input = dir.join("records.txt");
    let output = dir.join("output.zip");
    let invalid_payload = format!("alpha\n{}\n", "x".repeat(4_000));
    for (format, content, parallel) in [
        ("lines", invalid_payload.as_str(), false),
        ("lines", invalid_payload.as_str(), true),
        ("jsonl", "{\"text\":\"alpha\"}\n{\"text\":42}\n", false),
        ("json", "[\"alpha\", 42]", false),
        ("lines", "\n \n", false),
    ] {
        std::fs::write(&input, content).unwrap();
        std::fs::write(&output, b"existing output").unwrap();
        let mut command = bin();
        command
            .args(["--batch"])
            .arg(&input)
            .args(["--batch-format", format, "--batch-pack", "zip", "-f", "svg", "-o"])
            .arg(&output);
        if parallel {
            command.arg("--parallel");
        }
        let out = command.output().unwrap();
        assert!(!out.status.success(), "expected failure for {format}, parallel={parallel}");
        assert_eq!(std::fs::read(&output).unwrap(), b"existing output");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2, "temporary ZIP file was not removed");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_zip_batch_does_not_create_an_output_archive() {
    let dir = temporary_directory("zip_no_output");
    let input = dir.join("records.json");
    let output = dir.join("output.zip");
    std::fs::write(&input, "{invalid json").unwrap();
    let out = bin()
        .args(["--batch"])
        .arg(&input)
        .args(["--batch-format", "json", "--batch-pack", "zip", "-f", "svg", "-o"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!output.exists());
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn sequential_json_zip_contains_complete_ordered_payloads() {
    let dir = temporary_directory("json_zip");
    let input = dir.join("records.json");
    let output = dir.join("output.zip");
    std::fs::write(&input, r#"["alpha","beta"]"#).unwrap();
    let out = bin()
        .args(["--batch"])
        .arg(&input)
        .args(["--batch-format", "json", "--batch-pack", "zip", "-f", "svg", "-o"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let entries = stored_zip_entries(&std::fs::read(&output).unwrap());
    assert_eq!(entries.len(), 2);
    for ((name, payload), (expected_name, text)) in
        entries.iter().zip([("qr-0001.svg", "alpha"), ("qr-0002.svg", "beta")])
    {
        let expected = qrcode_rs::QrCode::new(text)
            .unwrap()
            .render::<qrcode_rs::render::svg::Color>()
            .dark_color(qrcode_rs::render::svg::Color("#000000"))
            .light_color(qrcode_rs::render::svg::Color("#ffffff"))
            .build();
        assert_eq!(name, expected_name);
        assert_eq!(payload, expected.as_bytes());
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn parallel_zip_chunks_keep_global_order_and_complete_payloads() {
    let dir = temporary_directory("zip_chunks");
    let input = dir.join("records.txt");
    let output = dir.join("output.zip");
    let payloads = (0..131).map(|index| format!("payload-{index:04}")).collect::<Vec<_>>();
    for format in ["lines", "json"] {
        let content = if format == "json" { serde_json::to_string(&payloads).unwrap() } else { payloads.join("\n") };
        std::fs::write(&input, content).unwrap();
        let result = bin()
            .args(["--batch"])
            .arg(&input)
            .args(["--batch-format", format, "--batch-pack", "zip", "--parallel", "-f", "svg", "-o"])
            .arg(&output)
            .output()
            .unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        let entries = stored_zip_entries(&std::fs::read(&output).unwrap());
        assert_eq!(entries.len(), payloads.len());
        for (index, ((name, actual), text)) in entries.iter().zip(&payloads).enumerate() {
            let expected = qrcode_rs::QrCode::new(text)
                .unwrap()
                .render::<qrcode_rs::render::svg::Color>()
                .dark_color(qrcode_rs::render::svg::Color("#000000"))
                .light_color(qrcode_rs::render::svg::Color("#ffffff"))
                .build();
            assert_eq!(name, &format!("qr-{:04}.svg", index + 1));
            assert_eq!(actual, expected.as_bytes());
        }
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn parallel_zip_error_after_completed_chunks_preserves_existing_output() {
    let dir = temporary_directory("zip_chunk_error");
    let input = dir.join("records.txt");
    let output = dir.join("output.zip");
    let mut payloads = (0..130).map(|index| format!("payload-{index:04}")).collect::<Vec<_>>();
    payloads.push("x".repeat(4_000));
    std::fs::write(&input, payloads.join("\n")).unwrap();
    std::fs::write(&output, b"existing output").unwrap();
    let result = bin()
        .args(["--batch"])
        .arg(&input)
        .args(["--batch-pack", "zip", "--parallel", "-f", "svg", "-o"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("data too long"));
    assert_eq!(std::fs::read(&output).unwrap(), b"existing output");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn grid_batch_writes_contact_sheet_png() {
    let stamp = time::SystemTime::now().duration_since(time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let image = std::env::temp_dir().join(format!("qrencodes_cli_grid_batch_{stamp}.png"));
    let list = std::env::temp_dir().join(format!("qrencodes_cli_grid_list_{stamp}.txt"));
    std::fs::write(&list, "alpha\nbeta\ngamma\n").unwrap();

    let out = bin()
        .args(["--batch"])
        .arg(&list)
        .args(["--batch-pack", "grid", "--grid-columns", "2", "-f", "png", "-o"])
        .arg(&image)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let bytes = std::fs::read(&image).unwrap();
    assert_eq!(&bytes[..8], &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

    std::fs::remove_file(list).unwrap();
    std::fs::remove_file(image).unwrap();
}

#[test]
fn grid_batch_rejects_excessive_sheet_size_and_preserves_output() {
    let dir = temporary_directory("grid_budget");
    let input = dir.join("records.txt");
    let output = dir.join("output.png");
    std::fs::write(&input, "alpha\n").unwrap();
    std::fs::write(&output, b"existing image").unwrap();
    let out = bin()
        .args(["--batch"])
        .arg(&input)
        .args(["--batch-pack", "grid", "--grid-columns", "1000000", "-f", "png", "--size", "1", "-o"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("PNG grid dimensions"));
    assert_eq!(std::fs::read(&output).unwrap(), b"existing image");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn validate_decodes_generated_png() {
    let stamp = time::SystemTime::now().duration_since(time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let path = std::env::temp_dir().join(format!("qrencodes_cli_validate_{stamp}.png"));
    let generate = bin().args(["-f", "png", "-o"]).arg(&path).arg("decode me").output().unwrap();
    assert!(generate.status.success(), "{}", String::from_utf8_lossy(&generate.stderr));

    let validate = bin().args(["validate", "--expect", "decode me"]).arg(&path).output().unwrap();
    assert!(validate.status.success(), "{}", String::from_utf8_lossy(&validate.stderr));
    assert!(String::from_utf8_lossy(&validate.stdout).contains("valid: decoded"));
    std::fs::remove_file(path).unwrap();
}
