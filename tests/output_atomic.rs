//! Atomic output behavior shared by the CLI's file-writing paths.

#![cfg(feature = "cli")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const OUTPUT_MODES: [&str; 4] = ["single", "grid", "zip", "directory"];

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("qrencodes_atomic_{name}_{}_{stamp}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn run(&self, mode: &str, destination: &Path) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_qrencodes"));
        if mode == "single" {
            command.args(["-f", "svg", "-o"]).arg(destination).arg("replacement");
        } else {
            let input = self.0.join("inputs.txt");
            fs::write(&input, b"replacement\n").unwrap();
            command.args(["--batch"]).arg(input).args(["--batch-pack", mode, "-f"]);
            command.arg(if mode == "grid" { "png" } else { "svg" });
            command.arg("-o").arg(destination);
        }
        command.output().unwrap()
    }

    fn destination(&self, mode: &str) -> (PathBuf, PathBuf) {
        let argument = self.0.join(mode);
        if mode == "directory" {
            fs::create_dir(&argument).unwrap();
            let file = argument.join("qr-0001.svg");
            (argument, file)
        } else {
            (argument.clone(), argument)
        }
    }

    fn assert_no_temporary_files(&self) {
        fn check(directory: &Path) {
            for entry in fs::read_dir(directory).unwrap() {
                let entry = entry.unwrap();
                assert!(!entry.file_name().to_string_lossy().starts_with(".qrencodes-"));
                if entry.file_type().unwrap().is_dir() {
                    check(&entry.path());
                }
            }
        }
        check(&self.0);
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
#[test]
fn every_file_output_path_preserves_existing_private_permissions() {
    let directory = TestDirectory::new("permissions");
    for mode in OUTPUT_MODES {
        let (argument, destination) = directory.destination(mode);
        fs::write(&destination, b"existing private output").unwrap();
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o600)).unwrap();
        let result = directory.run(mode, &argument);
        assert!(result.status.success(), "{mode}: {}", String::from_utf8_lossy(&result.stderr));
        assert_ne!(fs::read(&destination).unwrap(), b"existing private output");
        assert_eq!(fs::metadata(&destination).unwrap().permissions().mode() & 0o777, 0o600, "{mode}");
        directory.assert_no_temporary_files();
    }
}

#[test]
fn every_file_output_path_preserves_read_only_destinations() {
    let directory = TestDirectory::new("read-only");
    for mode in OUTPUT_MODES {
        let (argument, destination) = directory.destination(mode);
        fs::write(&destination, b"existing read-only output").unwrap();
        let original_permissions = fs::metadata(&destination).unwrap().permissions();
        let mut permissions = original_permissions.clone();
        permissions.set_readonly(true);
        fs::set_permissions(&destination, permissions).unwrap();
        let result = directory.run(mode, &argument);
        assert!(!result.status.success(), "{mode} replaced a read-only destination");
        assert_eq!(fs::read(&destination).unwrap(), b"existing read-only output", "{mode}");
        assert!(fs::metadata(&destination).unwrap().permissions().readonly(), "{mode}");
        directory.assert_no_temporary_files();
        fs::set_permissions(&destination, original_permissions).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn every_file_output_path_preserves_symlinks_and_their_targets() {
    let directory = TestDirectory::new("symlink");
    for mode in OUTPUT_MODES {
        let (argument, destination) = directory.destination(mode);
        let target = directory.0.join(format!("target-{mode}"));
        fs::write(&target, b"existing target output").unwrap();
        std::os::unix::fs::symlink(&target, &destination).unwrap();
        let result = directory.run(mode, &argument);
        assert!(!result.status.success(), "{mode} replaced or followed the output symlink");
        assert!(fs::symlink_metadata(&destination).unwrap().file_type().is_symlink(), "{mode}");
        assert_eq!(fs::read(&target).unwrap(), b"existing target output", "{mode}");
        directory.assert_no_temporary_files();
    }
}

#[cfg(unix)]
#[test]
fn output_files_can_be_written_through_a_parent_directory_symlink() {
    let directory = TestDirectory::new("parent-symlink");
    let target = directory.0.join("target-directory");
    fs::create_dir(&target).unwrap();
    let parent = directory.0.join("linked-directory");
    std::os::unix::fs::symlink(&target, &parent).unwrap();
    let destination = parent.join("output.svg");
    fs::write(&destination, b"existing private output").unwrap();
    fs::set_permissions(&destination, fs::Permissions::from_mode(0o600)).unwrap();
    let result = directory.run("single", &destination);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert!(fs::read(target.join("output.svg")).unwrap().starts_with(b"<?xml"));
    assert_eq!(fs::metadata(&destination).unwrap().permissions().mode() & 0o777, 0o600);
    assert!(fs::symlink_metadata(&parent).unwrap().file_type().is_symlink());
    directory.assert_no_temporary_files();
}

#[test]
fn stdout_output_remains_available_with_a_dash_destination() {
    let result =
        Command::new(env!("CARGO_BIN_EXE_qrencodes")).args(["-f", "svg", "-o", "-", "stdout"]).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert!(result.stdout.starts_with(b"<?xml"));
}
