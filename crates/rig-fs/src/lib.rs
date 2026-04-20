//! `rig-fs` — filesystem primitives.
//!
//! Atomic writes, home-dir resolution, path normalisation, and content
//! hashing. Every other crate that needs fs access goes through here so
//! higher layers (especially `rig-core`) stay pure.

#![forbid(unsafe_code)]

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use rig_core::source::Sha256;

pub mod pack;
pub use pack::{pack_dir, unpack_to_temp};

#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("home directory not discoverable")]
    NoHome,
    #[error("path expansion failed for `{0}`")]
    Expand(String),
    #[error("pack/unpack error: {0}")]
    Pack(String),
}

pub type FsResult<T> = Result<T, FsError>;

/// Write `bytes` to `path` atomically: write to a sibling `.tmp`, fsync,
/// rename. Creates parent directories as needed.
///
/// # Errors
/// Any underlying I/O failure is wrapped in [`FsError::Io`].
pub fn atomic_write(path: &Path, bytes: &[u8]) -> FsResult<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|source| FsError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
    }
    let tmp = tmp_sibling(path);
    {
        let mut f = fs::File::create(&tmp).map_err(|source| FsError::Io {
            path: tmp.clone(),
            source,
        })?;
        f.write_all(bytes).map_err(|source| FsError::Io {
            path: tmp.clone(),
            source,
        })?;
        f.sync_all().map_err(|source| FsError::Io {
            path: tmp.clone(),
            source,
        })?;
    }
    fs::rename(&tmp, path).map_err(|source| FsError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Read a file's bytes.
///
/// # Errors
/// Any underlying I/O failure is wrapped in [`FsError::Io`].
pub fn read(path: &Path) -> FsResult<Vec<u8>> {
    fs::read(path).map_err(|source| FsError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Hash a file's current contents.
///
/// # Errors
/// Any underlying I/O failure is wrapped in [`FsError::Io`].
pub fn sha_of(path: &Path) -> FsResult<Sha256> {
    Ok(Sha256::of(&read(path)?))
}

/// Remove a file if it exists. Missing files are not an error.
///
/// # Errors
/// Any non-`NotFound` I/O failure is wrapped in [`FsError::Io`].
pub fn remove_if_exists(path: &Path) -> FsResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(FsError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Current user's home directory.
///
/// # Errors
/// [`FsError::NoHome`] if `dirs::home_dir` returns `None`.
pub fn home_dir() -> FsResult<PathBuf> {
    dirs::home_dir().ok_or(FsError::NoHome)
}

/// Expand `~` and environment variables in a path string.
///
/// # Errors
/// [`FsError::Expand`] on malformed variable syntax.
pub fn expand(p: &str) -> FsResult<PathBuf> {
    shellexpand::full(p)
        .map(|s| PathBuf::from(s.as_ref()))
        .map_err(|_| FsError::Expand(p.to_owned()))
}

fn tmp_sibling(path: &Path) -> PathBuf {
    let mut name = path.file_name().map_or_else(
        || std::ffi::OsString::from(""),
        std::ffi::OsStr::to_os_string,
    );
    name.push(".rig-tmp");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_creates_parents_and_file() {
        let tmp = tempdir();
        let target = tmp.join("nested/dir/hello.txt");
        atomic_write(&target, b"hi").unwrap();
        assert_eq!(read(&target).unwrap(), b"hi");
        cleanup(&tmp);
    }

    #[test]
    fn atomic_write_overwrites_existing() {
        let tmp = tempdir();
        let p = tmp.join("f.txt");
        atomic_write(&p, b"one").unwrap();
        atomic_write(&p, b"two").unwrap();
        assert_eq!(read(&p).unwrap(), b"two");
        cleanup(&tmp);
    }

    #[test]
    fn sha_matches_content() {
        let tmp = tempdir();
        let p = tmp.join("f.txt");
        atomic_write(&p, b"abc").unwrap();
        assert_eq!(sha_of(&p).unwrap(), Sha256::of(b"abc"));
        cleanup(&tmp);
    }

    #[test]
    fn remove_missing_is_ok() {
        let tmp = tempdir();
        remove_if_exists(&tmp.join("nonexistent")).unwrap();
        cleanup(&tmp);
    }

    fn tempdir() -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "rig-fs-test-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        fs::create_dir_all(&base).unwrap();
        base
    }

    fn cleanup(p: &Path) {
        let _ = fs::remove_dir_all(p);
    }

    fn rand_suffix() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(0)
    }
}
