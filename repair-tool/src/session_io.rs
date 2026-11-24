//! Shared bounded native file I/O. Simulation state is validated by each host.
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
pub const MAX_SNAPSHOT_BYTES: usize = 8 * 1024 * 1024;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
fn check_nesting(bytes: &[u8]) -> Result<(), String> {
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for &byte in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'[' | b'{' => {
                    depth += 1;
                    if depth > 64 {
                        return Err("snapshot nesting exceeds 64 levels".into());
                    }
                }
                b']' | b'}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    Ok(())
}

pub(crate) fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    if !fs::metadata(path)
        .map_err(|e| format!("cannot inspect snapshot: {}", e.kind()))?
        .is_file()
    {
        return Err("snapshot must be a regular file".into());
    }
    let file = File::open(path).map_err(|e| format!("cannot open snapshot: {}", e.kind()))?;
    if !file
        .metadata()
        .map_err(|e| format!("cannot inspect snapshot: {}", e.kind()))?
        .is_file()
    {
        return Err("snapshot must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_SNAPSHOT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read snapshot: {}", e.kind()))?;
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err("snapshot exceeds 8388608 bytes".into());
    }
    check_nesting(&bytes)?;
    Ok(bytes)
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[derive(Debug)]
pub enum SaveError {
    BeforeReplacement(String),
    DurabilityUncertain(String),
}
impl From<String> for SaveError {
    fn from(message: String) -> Self {
        Self::BeforeReplacement(message)
    }
}
impl From<&str> for SaveError {
    fn from(message: &str) -> Self {
        Self::BeforeReplacement(message.into())
    }
}
impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BeforeReplacement(message) | Self::DurabilityUncertain(message) => {
                f.write_str(message)
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SaveStage {
    Write,
    FileSync,
    Rename,
    DirectorySync,
}

pub(crate) fn replace(path: &Path, bytes: &[u8]) -> Result<(), SaveError> {
    replace_with(path, bytes, |_| Ok(()))
}

// The stage callback tests disk failures at the actual commit boundary.
pub(crate) fn replace_with(
    path: &Path,
    bytes: &[u8],
    stage: impl Fn(SaveStage) -> std::io::Result<()>,
) -> Result<(), SaveError> {
    if !cfg!(any(
        target_os = "linux",
        all(target_os = "macos", target_arch = "aarch64")
    )) {
        return Err(
            "atomic session replacement requires Linux or macOS ARM (qualified local APFS)".into(),
        );
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let directory =
        File::open(parent).map_err(|e| format!("cannot open snapshot directory: {}", e.kind()))?;
    let mut created = None;
    for _ in 0..64 {
        let temp = parent.join(format!(
            ".repair-session-{}-{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&temp) {
            Ok(file) => {
                created = Some((Temporary(temp), file));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(format!("cannot create snapshot temporary file: {}", e.kind()).into());
            }
        }
    }
    let (temp, mut file) = created.ok_or("cannot create unique snapshot temporary file")?;
    stage(SaveStage::Write)
        .and_then(|_| file.write_all(bytes))
        .and_then(|_| stage(SaveStage::FileSync))
        .and_then(|_| file.sync_all())
        .map_err(|e| {
            format!(
                "cannot write and sync snapshot temporary file: {}",
                e.kind()
            )
        })?;
    stage(SaveStage::Rename)
        .and_then(|_| fs::rename(&temp.0, path))
        .map_err(|e| format!("cannot atomically replace snapshot: {}", e.kind()))?;
    stage(SaveStage::DirectorySync).and_then(|_| directory.sync_all()).map_err(|e| SaveError::DurabilityUncertain(format!("snapshot replacement committed; durability uncertain because directory sync failed: {}", e.kind())))?;
    Ok(())
}

#[cfg(all(
    test,
    any(target_os = "linux", all(target_os = "macos", target_arch = "aarch64"))
))]
mod tests {
    use super::*;
    use std::{cell::RefCell, os::unix::fs::PermissionsExt};

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "deedwake-atomic-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn replacement_uses_private_sibling_and_syncs_both_commit_boundaries() {
        let scratch = Scratch::new();
        let path = scratch.0.join("snapshot.json");
        fs::write(&path, b"previous").unwrap();
        let stages = RefCell::new(Vec::new());
        replace_with(&path, b"replacement", |stage| {
            stages.borrow_mut().push(stage);
            if stage != SaveStage::DirectorySync {
                assert_eq!(fs::read(&path).unwrap(), b"previous");
                let siblings: Vec<_> = fs::read_dir(&scratch.0)
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .filter(|entry| entry != &path)
                    .collect();
                assert_eq!(siblings.len(), 1);
                // Mode bits do not prove other-user denial on ownership-disabled volumes.
                assert_eq!(
                    fs::metadata(&siblings[0]).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            } else {
                assert_eq!(fs::read(&path).unwrap(), b"replacement");
            }
            Ok(())
        })
        .unwrap();
        assert!(
            *stages.borrow()
                == [
                    SaveStage::Write,
                    SaveStage::FileSync,
                    SaveStage::Rename,
                    SaveStage::DirectorySync
                ]
        );
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
    }

    #[test]
    fn failures_preserve_precommit_bytes_but_report_postcommit_uncertainty() {
        let scratch = Scratch::new();
        let path = scratch.0.join("snapshot.json");
        for failed in [
            SaveStage::Write,
            SaveStage::FileSync,
            SaveStage::Rename,
            SaveStage::DirectorySync,
        ] {
            fs::write(&path, b"previous").unwrap();
            let error = replace_with(&path, b"replacement", |stage| {
                if stage == failed {
                    Err(std::io::Error::other("injected disk failure"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            let committed = failed == SaveStage::DirectorySync;
            assert_eq!(
                matches!(error, SaveError::DurabilityUncertain(_)),
                committed
            );
            assert_eq!(
                fs::read(&path).unwrap(),
                if committed {
                    b"replacement".as_slice()
                } else {
                    b"previous".as_slice()
                }
            );
            assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
        }
    }
}
