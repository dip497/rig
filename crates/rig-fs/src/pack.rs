//! Deterministic `.rig` tarballs: gzipped tar with sorted entries,
//! zeroed mtime/uid/gid, normalised mode. Same input dir → byte-identical
//! output, so the tarball SHA can pin a unit in the lockfile.

use std::fs;
use std::path::{Path, PathBuf};

use flate2::Compression;
use tar::{Builder, HeaderMode};
use tempfile::TempDir;

use super::{FsError, FsResult};

/// Pack `src` directory into a deterministic gzipped tar at `out`.
///
/// Contract:
/// - Entries are sorted by relative path (filesystem walk order is
///   non-deterministic).
/// - `HeaderMode::Deterministic` zeros `mtime`, `uid`, `gid`, `uname`,
///   `gname` and normalises file modes.
/// - Gzip header `mtime` is set to `0`.
/// - Symlinks and hardlinks are skipped (warning quiet — not portable).
/// - Files and directories only.
///
/// # Errors
/// Any I/O failure is wrapped in [`FsError::Io`] or [`FsError::Pack`].
pub fn pack_dir(src: &Path, out: &Path) -> FsResult<()> {
    if !src.is_dir() {
        return Err(FsError::Pack(format!(
            "source `{}` is not a directory",
            src.display()
        )));
    }

    let mut entries: Vec<PathBuf> = Vec::new();
    walk_sorted(src, &mut entries).map_err(|source| FsError::Io {
        path: src.to_path_buf(),
        source,
    })?;
    entries.sort();

    let mut buf: Vec<u8> = Vec::new();
    {
        let gz = flate2::GzBuilder::new()
            .mtime(0)
            .write(&mut buf, Compression::default());
        let mut tar = Builder::new(gz);
        tar.mode(HeaderMode::Deterministic);
        tar.follow_symlinks(false);

        for abs in entries {
            let rel = abs
                .strip_prefix(src)
                .map_err(|_| FsError::Pack(format!("strip_prefix failed for {}", abs.display())))?;
            let meta = fs::symlink_metadata(&abs).map_err(|source| FsError::Io {
                path: abs.clone(),
                source,
            })?;
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_file() {
                let mut f = fs::File::open(&abs).map_err(|source| FsError::Io {
                    path: abs.clone(),
                    source,
                })?;
                tar.append_file(rel, &mut f).map_err(|source| FsError::Io {
                    path: abs.clone(),
                    source,
                })?;
            }
        }

        tar.finish().map_err(|source| FsError::Io {
            path: src.to_path_buf(),
            source,
        })?;
        let gz_inner = tar.into_inner().map_err(|source| FsError::Io {
            path: src.to_path_buf(),
            source,
        })?;
        gz_inner.finish().map_err(|source| FsError::Io {
            path: out.to_path_buf(),
            source,
        })?;
    }

    super::atomic_write(out, &buf)
}

/// Extract `archive` (gzipped tar) into a fresh [`TempDir`]. Rejects
/// entries with `..` path components (path-traversal guard provided by
/// `tar::Entry::unpack_in`).
///
/// # Errors
/// Any I/O or tar failure wrapped in [`FsError::Io`] / [`FsError::Pack`].
pub fn unpack_to_temp(archive: &Path) -> FsResult<TempDir> {
    let bytes = super::read(archive)?;
    let dir = tempfile::Builder::new()
        .prefix("rig-unpack-")
        .tempdir()
        .map_err(|source| FsError::Io {
            path: archive.to_path_buf(),
            source,
        })?;

    let gz = flate2::read::GzDecoder::new(&bytes[..]);
    let mut ar = tar::Archive::new(gz);
    for entry in ar.entries().map_err(|source| FsError::Io {
        path: archive.to_path_buf(),
        source,
    })? {
        let mut entry = entry.map_err(|source| FsError::Io {
            path: archive.to_path_buf(),
            source,
        })?;
        let ok = entry.unpack_in(dir.path()).map_err(|source| FsError::Io {
            path: archive.to_path_buf(),
            source,
        })?;
        if !ok {
            return Err(FsError::Pack(format!(
                "refused unsafe entry in `{}`",
                archive.display()
            )));
        }
    }
    Ok(dir)
}

fn walk_sorted(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut local: Vec<_> = fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    local.sort_by_key(std::fs::DirEntry::path);
    for entry in local {
        let p = entry.path();
        let ft = entry.file_type()?;
        if ft.is_dir() {
            walk_sorted(&p, out)?;
        } else if ft.is_file() {
            out.push(p);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(tag: &str) -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let p = std::env::temp_dir().join(format!("rig-fs-pack-{tag}-{}-{n}", std::process::id()));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn fixture() -> PathBuf {
        let d = tempdir("fx");
        fs::create_dir_all(d.join("sub")).unwrap();
        fs::write(d.join("SKILL.md"), b"---\nname: s\n---\nbody\n").unwrap();
        fs::write(d.join("sub/note.txt"), b"hello").unwrap();
        d
    }

    #[test]
    fn pack_is_deterministic() {
        let src = fixture();
        let out_dir = tempdir("out");
        let a = out_dir.join("a.rig");
        let b = out_dir.join("b.rig");
        pack_dir(&src, &a).unwrap();
        pack_dir(&src, &b).unwrap();
        assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
        fs::remove_dir_all(&src).ok();
        fs::remove_dir_all(&out_dir).ok();
    }

    #[test]
    fn pack_unpack_roundtrip() {
        let src = fixture();
        let out_dir = tempdir("rt");
        let archive = out_dir.join("x.rig");
        pack_dir(&src, &archive).unwrap();

        let extracted = unpack_to_temp(&archive).unwrap();
        let skill = extracted.path().join("SKILL.md");
        let note = extracted.path().join("sub/note.txt");
        assert_eq!(fs::read(&skill).unwrap(), b"---\nname: s\n---\nbody\n");
        assert_eq!(fs::read(&note).unwrap(), b"hello");
        fs::remove_dir_all(&src).ok();
        fs::remove_dir_all(&out_dir).ok();
    }

    #[test]
    fn pack_rejects_non_directory() {
        let d = tempdir("nf");
        let f = d.join("lone.txt");
        fs::write(&f, b"x").unwrap();
        let out = d.join("out.rig");
        assert!(matches!(pack_dir(&f, &out), Err(FsError::Pack(_))));
        fs::remove_dir_all(&d).ok();
    }
}
