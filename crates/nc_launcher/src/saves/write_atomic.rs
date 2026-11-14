use std::io;
use std::path::{Path, PathBuf};

const STAGING_SUFFIX: &str = ".tmp";

/// Writes beside the target and renames over it, so a reader sees the old file or the new one.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let staging = staging_path(path);
    std::fs::write(&staging, bytes)?;
    std::fs::rename(&staging, path)
}

/// A path in `dir` named `stem.extension` that does not exist yet, numbered when it is taken.
pub fn unused_path(dir: &Path, stem: &str, extension: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{extension}"));
    if !first.exists() {
        return first;
    }
    (2_u32..)
        .map(|count| dir.join(format!("{stem}-{count}.{extension}")))
        .find(|path| !path.exists())
        .unwrap_or(first)
}

pub fn unix_seconds_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

fn staging_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(STAGING_SUFFIX);
    path.with_file_name(name)
}
