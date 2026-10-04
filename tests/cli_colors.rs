//! CLI hex-color spelling and rendering compatibility.

#![cfg(feature = "cli")]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn render(format: &str, text: &str, dark: &str, light: &str, invert: bool) -> Vec<u8> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_qrencodes"));
    command.args([text, "-f", format, "--dark", dark, "--light", light]);
    if format == "png" {
        command.args(["--size", "1"]);
    }
    if invert {
        command.arg("--invert");
    }
    let result = command.output().unwrap();
    assert!(result.status.success(), "{format}: {}", String::from_utf8_lossy(&result.stderr));
    result.stdout
}

#[test]
fn bare_hex_markup_colors_match_their_css_prefixed_forms() {
    for format in ["svg", "html"] {
        for (dark, light) in [("AbC", "fFf"), ("123456", "AbCdEf")] {
            for invert in [false, true] {
                let actual = render(format, "alpha", dark, light, invert);
                let expected = render(format, "alpha", &format!("#{dark}"), &format!("#{light}"), invert);
                assert_eq!(actual, expected, "{format}, {dark}, {light}, invert={invert}");
                let markup = String::from_utf8(actual).unwrap();
                assert!(markup.contains(&format!("#{dark}")));
                assert!(markup.contains(&format!("#{light}")));
            }
        }
    }
}

#[test]
fn existing_markup_css_case_and_short_forms_are_preserved() {
    for format in ["svg", "html"] {
        let markup = String::from_utf8(render(format, "alpha", "#AbC", "#dEf012", false)).unwrap();
        assert!(markup.contains("#AbC"));
        assert!(markup.contains("#dEf012"));
        assert!(!markup.contains("#aabbcc"));
    }
}

#[test]
fn non_markup_formats_keep_existing_bare_hex_behavior() {
    for format in ["png", "ansi", "eps", "pdf"] {
        for invert in [false, true] {
            assert_eq!(
                render(format, "alpha", "123456", "AbCdEf", invert),
                render(format, "alpha", "#123456", "#AbCdEf", invert),
                "{format}, invert={invert}"
            );
        }
    }
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("qrencodes-color-{}-{stamp}", std::process::id()));
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
fn parallel_directory_markup_colors_match_prefixed_single_outputs() {
    let directory = TestDirectory::new();
    let input = directory.0.join("records");
    fs::write(&input, "alpha\nbeta\n").unwrap();
    for format in ["svg", "html"] {
        let output = directory.0.join(format);
        let result = Command::new(env!("CARGO_BIN_EXE_qrencodes"))
            .args(["--batch"])
            .arg(&input)
            .args(["--parallel", "-f", format, "--dark", "AbC", "--light", "123456", "--invert", "-o"])
            .arg(&output)
            .output()
            .unwrap();
        assert!(result.status.success(), "{format}: {}", String::from_utf8_lossy(&result.stderr));
        assert_eq!(fs::read_dir(&output).unwrap().count(), 2);
        for (index, text) in ["alpha", "beta"].into_iter().enumerate() {
            let path = output.join(format!("qr-{:04}.{format}", index + 1));
            assert_eq!(fs::read(path).unwrap(), render(format, text, "#AbC", "#123456", true));
        }
    }
}
