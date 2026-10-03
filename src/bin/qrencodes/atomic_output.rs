//! Atomic replacement for regular CLI output files.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

static TEMP_OUTPUT_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

pub(super) struct AtomicOutputFile {
    file: Option<BufWriter<File>>,
    temporary_path: PathBuf,
    output_path: PathBuf,
    committed: bool,
    write_failed: bool,
}

impl AtomicOutputFile {
    pub(super) fn create(path: &Path) -> io::Result<Self> {
        let permissions = match fs::symlink_metadata(path) {
            Ok(metadata) => {
                if !metadata.is_file() {
                    return Err(io::Error::new(io::ErrorKind::InvalidInput, "output must be a regular file"));
                }
                let permissions = metadata.permissions();
                if permissions.readonly() {
                    return Err(io::Error::new(io::ErrorKind::PermissionDenied, "output file is read-only"));
                }
                Some(permissions)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
        for _ in 0..128 {
            let sequence = TEMP_OUTPUT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let temporary_name = format!(".qrencodes-{}-{sequence}.tmp", std::process::id());
            // Generated names are ASCII. Skip case aliases on every platform so
            // a case-insensitive volume cannot expose the destination early.
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case(&temporary_name))
            {
                continue;
            }
            let temporary_path = parent.join(temporary_name);
            let mut options = File::options();
            options.write(true).create_new(true);
            // Give an existing private destination's temporary file its private
            // mode from creation, rather than relying on the current umask.
            #[cfg(unix)]
            if let Some(permissions) = &permissions {
                options.mode(permissions.mode());
            }
            match options.open(&temporary_path) {
                Ok(file) => {
                    let output = Self {
                        file: Some(BufWriter::new(file)),
                        temporary_path,
                        output_path: path.to_owned(),
                        committed: false,
                        write_failed: false,
                    };
                    // The umask may have removed permission bits. Restore the
                    // existing permissions before writing any payload bytes.
                    if let Some(permissions) = &permissions {
                        output
                            .file
                            .as_ref()
                            .expect("new output has a file")
                            .get_ref()
                            .set_permissions(permissions.clone())?;
                    }
                    return Ok(output);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(io::ErrorKind::AlreadyExists, "could not create a temporary output file"))
    }

    pub(super) fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.write_failed {
            return Err(io::Error::other("an earlier output write failed"));
        }
        let result = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "output file is already closed"))?
            .write_all(bytes);
        self.write_failed = result.is_err();
        result
    }

    pub(super) fn finish(mut self) -> io::Result<()> {
        if self.write_failed {
            return Err(io::Error::other("an earlier output write failed"));
        }
        let mut file = self
            .file
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "output file is already closed"))?;
        file.flush()?;
        // Close the handle before rename and cleanup, including on Windows.
        drop(file);
        fs::rename(&self.temporary_path, &self.output_path)?;
        self.committed = true;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn temporary_path(&self) -> &Path {
        &self.temporary_path
    }
}

impl Drop for AtomicOutputFile {
    fn drop(&mut self) {
        drop(self.file.take());
        if !self.committed {
            let _ = fs::remove_file(&self.temporary_path);
        }
    }
}

pub(super) fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut output = AtomicOutputFile::create(path)?;
    output.write_all(bytes)?;
    output.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(name: &str) -> Self {
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!("qrcode-atomic-{name}-{}-{stamp}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn output(&self) -> PathBuf {
            self.0.join("output")
        }

        fn assert_no_temporary_files(&self) {
            assert!(
                fs::read_dir(&self.0).unwrap().all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".qrencodes-"))
            );
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn replaces_the_destination_only_after_finishing() {
        let directory = TestDirectory::new("success");
        let path = directory.output();
        fs::write(&path, b"previous").unwrap();
        let mut output = AtomicOutputFile::create(&path).unwrap();
        assert_eq!(output.temporary_path().parent(), path.parent());
        output.write_all(b"replacement").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"previous");
        output.finish().unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
        directory.assert_no_temporary_files();
    }

    #[test]
    fn unfinished_output_preserves_existing_content_and_cleans_up() {
        let directory = TestDirectory::new("unfinished");
        let path = directory.output();
        fs::write(&path, b"previous").unwrap();
        let mut output = AtomicOutputFile::create(&path).unwrap();
        output.write_all(b"partial").unwrap();
        drop(output);
        assert_eq!(fs::read(&path).unwrap(), b"previous");
        directory.assert_no_temporary_files();
    }

    #[test]
    fn unfinished_new_output_does_not_create_the_destination() {
        let directory = TestDirectory::new("unfinished-new");
        let path = directory.output();
        let mut output = AtomicOutputFile::create(&path).unwrap();
        output.write_all(b"partial").unwrap();
        assert!(!path.exists());
        drop(output);
        assert!(!path.exists());
        directory.assert_no_temporary_files();
    }

    #[test]
    fn temporary_name_case_alias_is_not_published_before_finishing() {
        const CHILD_FLAG: &str = "QRCODE_TEST_ATOMIC_CASE_ALIAS_CHILD";
        if std::env::var_os(CHILD_FLAG).is_none() {
            // Run only this test in a fresh process so no parallel test can
            // consume sequence zero and make the alias regression pass by luck.
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "atomic_output::tests::temporary_name_case_alias_is_not_published_before_finishing",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(CHILD_FLAG, "1")
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "child failed:\n{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"), "child test filter matched no test");
            return;
        }

        assert_eq!(TEMP_OUTPUT_SEQUENCE.load(Ordering::SeqCst), 0);
        let directory = TestDirectory::new("case-alias");
        let destination_name = format!(".QRENCODES-{}-0.TMP", std::process::id());
        let path = directory.0.join(&destination_name);
        let mut output = AtomicOutputFile::create(&path).unwrap();
        assert!(
            !output.temporary_path().file_name().unwrap().to_str().unwrap().eq_ignore_ascii_case(&destination_name)
        );
        assert!(!path.exists(), "destination became visible during temporary-file creation");
        output.write_all(b"replacement").unwrap();
        assert!(!path.exists(), "destination became visible before commit");
        output.finish().unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
        directory.assert_no_temporary_files();
    }

    fn replace_writer_with_read_only_handle(output: &mut AtomicOutputFile) {
        drop(output.file.take());
        output.file = Some(BufWriter::new(File::open(output.temporary_path()).unwrap()));
    }

    #[test]
    fn failed_direct_write_cannot_commit_partial_output() {
        let directory = TestDirectory::new("write-failure");
        let path = directory.output();
        fs::write(&path, b"previous").unwrap();
        let mut output = AtomicOutputFile::create(&path).unwrap();
        replace_writer_with_read_only_handle(&mut output);
        let bytes = vec![0; output.file.as_ref().unwrap().capacity() + 1];
        assert!(output.write_all(&bytes).is_err());
        assert!(output.finish().is_err());
        assert_eq!(fs::read(&path).unwrap(), b"previous");
        directory.assert_no_temporary_files();
    }

    #[test]
    fn failed_flush_preserves_existing_content_and_cleans_up() {
        let directory = TestDirectory::new("flush-failure");
        let path = directory.output();
        fs::write(&path, b"previous").unwrap();
        let mut output = AtomicOutputFile::create(&path).unwrap();
        replace_writer_with_read_only_handle(&mut output);
        output.write_all(b"buffered").unwrap();
        assert!(output.finish().is_err());
        assert_eq!(fs::read(&path).unwrap(), b"previous");
        directory.assert_no_temporary_files();
    }

    #[test]
    fn failed_rename_preserves_the_destination_and_cleans_up() {
        let directory = TestDirectory::new("rename-failure");
        let path = directory.output();
        let mut output = AtomicOutputFile::create(&path).unwrap();
        output.write_all(b"replacement").unwrap();
        fs::create_dir(&path).unwrap();
        assert!(output.finish().is_err());
        assert!(path.is_dir());
        directory.assert_no_temporary_files();
    }

    #[test]
    fn existing_directory_destinations_are_preserved() {
        let directory = TestDirectory::new("directory");
        let path = directory.output();
        fs::create_dir(&path).unwrap();
        let child = path.join("previous");
        fs::write(&child, b"previous").unwrap();
        assert!(matches!(write(&path, b"replacement"), Err(error) if error.kind() == io::ErrorKind::InvalidInput));
        assert_eq!(fs::read(&child).unwrap(), b"previous");
        directory.assert_no_temporary_files();
    }

    #[cfg(unix)]
    #[test]
    fn existing_private_permissions_are_set_before_writing() {
        let directory = TestDirectory::new("private-mode");
        let path = directory.output();
        fs::write(&path, b"previous").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut output = AtomicOutputFile::create(&path).unwrap();
        assert_eq!(fs::metadata(output.temporary_path()).unwrap().permissions().mode() & 0o777, 0o600);
        output.write_all(b"replacement").unwrap();
        output.finish().unwrap();
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        directory.assert_no_temporary_files();
    }

    #[cfg(unix)]
    #[test]
    fn new_files_follow_the_current_umask() {
        let directory = TestDirectory::new("new-mode");
        let probe = directory.0.join("probe");
        File::create(&probe).unwrap();
        let expected_mode = fs::metadata(&probe).unwrap().permissions().mode() & 0o777;
        let path = directory.output();
        write(&path, b"new output").unwrap();
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, expected_mode);
        directory.assert_no_temporary_files();
    }

    #[test]
    fn read_only_destinations_are_preserved() {
        let directory = TestDirectory::new("read-only");
        let path = directory.output();
        fs::write(&path, b"previous").unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        let original_permissions = permissions.clone();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).unwrap();
        assert!(matches!(write(&path, b"replacement"), Err(error) if error.kind() == io::ErrorKind::PermissionDenied));
        assert_eq!(fs::read(&path).unwrap(), b"previous");
        directory.assert_no_temporary_files();
        fs::set_permissions(&path, original_permissions).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_destinations_and_their_targets_are_preserved() {
        let directory = TestDirectory::new("symlink");
        let target = directory.0.join("target");
        fs::write(&target, b"previous").unwrap();
        let path = directory.output();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(matches!(write(&path, b"replacement"), Err(error) if error.kind() == io::ErrorKind::InvalidInput));
        assert!(fs::symlink_metadata(&path).unwrap().file_type().is_symlink());
        assert_eq!(fs::read(&target).unwrap(), b"previous");
        directory.assert_no_temporary_files();
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_destinations_are_preserved() {
        let directory = TestDirectory::new("dangling-symlink");
        let target = directory.0.join("missing-target");
        let path = directory.output();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(matches!(write(&path, b"replacement"), Err(error) if error.kind() == io::ErrorKind::InvalidInput));
        assert!(fs::symlink_metadata(&path).unwrap().file_type().is_symlink());
        assert!(!target.exists());
        directory.assert_no_temporary_files();
    }

    #[cfg(unix)]
    #[test]
    fn parent_directory_symlinks_allow_atomic_replacement() {
        let directory = TestDirectory::new("parent-symlink");
        let target = directory.0.join("target-directory");
        fs::create_dir(&target).unwrap();
        let parent = directory.0.join("linked-directory");
        std::os::unix::fs::symlink(&target, &parent).unwrap();
        let path = parent.join("output");
        fs::write(&path, b"previous").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        write(&path, b"replacement").unwrap();
        assert_eq!(fs::read(target.join("output")).unwrap(), b"replacement");
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert!(fs::symlink_metadata(&parent).unwrap().file_type().is_symlink());
        assert_eq!(fs::read_dir(&target).unwrap().count(), 1);
        directory.assert_no_temporary_files();
    }
}
