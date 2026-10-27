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
    if !cfg!(target_os = "linux") {
        return Err("atomic session replacement is supported only on Linux".into());
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
