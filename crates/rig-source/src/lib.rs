//! `rig-source` — resolves [`Source`] refs into on-disk bytes plus a
//! detected unit type.
//!
//! Output is agent-neutral: a [`NativeLayout`] plus the detected
//! [`UnitType`]. Callers (CLI, sync engine) pick the right adapter
//! converter to turn it into a canonical [`Unit`].
//!
//! First wedge: local paths only. GitHub/git/npm/marketplace land when
//! a caller needs them.
//!
//! [`Source`]: rig_core::source::Source
//! [`NativeLayout`]: rig_core::converter::NativeLayout
//! [`UnitType`]: rig_core::unit::UnitType
//! [`Unit`]: rig_core::unit::Unit

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use rig_core::converter::{NativeFile, NativeLayout};
use rig_core::source::{Sha256, Source};
use rig_core::unit::UnitType;

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("source `{0}` not supported yet")]
    Unsupported(String),
    #[error("local path `{0}` does not exist")]
    MissingPath(PathBuf),
    #[error("could not detect unit type at `{0}` (expected e.g. SKILL.md)")]
    Undetected(PathBuf),
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub type FetchResult<T> = Result<T, FetchError>;

#[derive(Debug, Clone)]
pub struct Fetched {
    pub source: Source,
    /// SHA of the fetched bytes in canonical (sorted) order.
    pub source_sha: Sha256,
    pub native: NativeLayout,
    /// `None` when the source bytes alone do not disambiguate the unit
    /// type (e.g. a bare markdown file could be a rule, command, or
    /// subagent). Callers pass an override in that case.
    pub detected: Option<UnitType>,
}

/// Resolve a [`Source`] into bytes on disk.
///
/// # Errors
/// - [`FetchError::Unsupported`] for non-local schemes in M1.
/// - [`FetchError::MissingPath`] if a local path does not exist.
/// - [`FetchError::Undetected`] if the unit type cannot be inferred.
/// - [`FetchError::Io`] for filesystem failures.
pub fn fetch(source: &Source) -> FetchResult<Fetched> {
    match source {
        Source::Local { path } => fetch_local(source, Path::new(path)),
        Source::Github { .. }
        | Source::Git { .. }
        | Source::Npm { .. }
        | Source::Marketplace { .. } => Err(FetchError::Unsupported(source.to_string())),
    }
}

fn fetch_local(source: &Source, path: &Path) -> FetchResult<Fetched> {
    if !path.exists() {
        return Err(FetchError::MissingPath(path.to_path_buf()));
    }

    if is_tarball(path) {
        return fetch_tarball(source, path);
    }

    let (root, detected) = detect(path)?;
    let files = if path.is_file() {
        vec![path.to_path_buf()]
    } else {
        collect(&root).map_err(|source| FetchError::Io {
            path: root.clone(),
            source,
        })?
    };

    let (native, hash_input) = read_files(&files, &root, path.is_file())?;

    Ok(Fetched {
        source: source.clone(),
        source_sha: Sha256::of(&hash_input),
        native: NativeLayout { files: native },
        detected,
    })
}

fn is_tarball(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    name.ends_with(".rig") || name.ends_with(".tar.gz") || name.ends_with(".tgz")
}

fn fetch_tarball(source: &Source, path: &Path) -> FetchResult<Fetched> {
    let archive_bytes = std::fs::read(path).map_err(|s| FetchError::Io {
        path: path.to_path_buf(),
        source: s,
    })?;
    let source_sha = Sha256::of(&archive_bytes);

    let temp = rig_fs::unpack_to_temp(path).map_err(|e| FetchError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
    })?;

    let (root, detected) = detect(temp.path())?;
    let files = collect(&root).map_err(|source| FetchError::Io {
        path: root.clone(),
        source,
    })?;
    let (native, _) = read_files(&files, &root, false)?;

    Ok(Fetched {
        source: source.clone(),
        source_sha,
        native: NativeLayout { files: native },
        detected,
    })
}

fn read_files(
    files: &[PathBuf],
    root: &Path,
    is_single: bool,
) -> FetchResult<(Vec<NativeFile>, Vec<u8>)> {
    let mut native = Vec::with_capacity(files.len());
    let mut hash_input = Vec::new();
    for p in files {
        let rel = if is_single {
            p.file_name().unwrap().to_string_lossy().into_owned()
        } else {
            p.strip_prefix(root)
                .unwrap_or(p)
                .to_string_lossy()
                .into_owned()
        };
        let bytes = std::fs::read(p).map_err(|source| FetchError::Io {
            path: p.clone(),
            source,
        })?;
        hash_input.extend_from_slice(rel.as_bytes());
        hash_input.push(0);
        hash_input.extend_from_slice(&bytes);
        hash_input.push(0);
        native.push(NativeFile {
            relative_path: rel,
            bytes,
        });
    }
    Ok((native, hash_input))
}

/// Return (`root_dir`, detected type). If `path` is a file (e.g. a
/// direct `SKILL.md`), root is its parent. Single `.md` files that
/// aren't `SKILL.md` are type-ambiguous — the caller must supply a
/// hint via `--as`.
fn detect(path: &Path) -> FetchResult<(PathBuf, Option<UnitType>)> {
    let (root, file_hint, is_single_file) = if path.is_file() {
        (
            path.parent().unwrap_or(Path::new(".")).to_path_buf(),
            Some(path.file_name().unwrap().to_string_lossy().into_owned()),
            true,
        )
    } else {
        (path.to_path_buf(), None, false)
    };

    if root.join("SKILL.md").exists() || file_hint.as_deref() == Some("SKILL.md") {
        return Ok((root, Some(UnitType::Skill)));
    }

    if is_single_file {
        // Single markdown file → caller hints the type.
        return Ok((root, None));
    }

    // Directory with no SKILL.md and no recognised structure.
    Err(FetchError::Undetected(path.to_path_buf()))
}

fn collect(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk(dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        let ft = entry.file_type()?;
        if ft.is_dir() {
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            walk(&p, out)?;
        } else if ft.is_file() {
            out.push(p);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tempdir(tag: &str) -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let p = std::env::temp_dir().join(format!("rig-source-{tag}-{}-{n}", std::process::id()));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn fetches_local_skill_dir() {
        let dir = tempdir("skill");
        fs::write(
            dir.join("SKILL.md"),
            "---\nname: foo\ndescription: d\n---\nbody\n",
        )
        .unwrap();

        let src = Source::Local {
            path: dir.to_string_lossy().into_owned(),
        };
        let f = fetch(&src).unwrap();
        assert_eq!(f.detected, Some(UnitType::Skill));
        assert_eq!(f.native.files.len(), 1);
        assert_eq!(f.native.files[0].relative_path, "SKILL.md");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn single_md_file_is_undetected() {
        let dir = tempdir("md");
        let p = dir.join("my-rule.md");
        fs::write(&p, "---\nname: my-rule\n---\nbody\n").unwrap();
        let src = Source::Local {
            path: p.to_string_lossy().into_owned(),
        };
        let f = fetch(&src).unwrap();
        assert!(f.detected.is_none());
        assert_eq!(f.native.files.len(), 1);
        assert_eq!(f.native.files[0].relative_path, "my-rule.md");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn undetected_for_empty_dir() {
        let dir = tempdir("empty");
        let src = Source::Local {
            path: dir.to_string_lossy().into_owned(),
        };
        assert!(matches!(fetch(&src), Err(FetchError::Undetected(_))));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fetches_tarball() {
        let src_dir = tempdir("tb-src");
        fs::write(
            src_dir.join("SKILL.md"),
            "---\nname: tb\ndescription: d\n---\nbody\n",
        )
        .unwrap();
        let out_dir = tempdir("tb-out");
        let archive = out_dir.join("tb.rig");
        rig_fs::pack_dir(&src_dir, &archive).unwrap();

        let src = Source::Local {
            path: archive.to_string_lossy().into_owned(),
        };
        let f = fetch(&src).unwrap();
        assert_eq!(f.detected, Some(UnitType::Skill));
        assert_eq!(f.native.files.len(), 1);
        assert_eq!(f.native.files[0].relative_path, "SKILL.md");

        // Deterministic source_sha: same tarball → same sha.
        let g = fetch(&src).unwrap();
        assert_eq!(f.source_sha, g.source_sha);

        fs::remove_dir_all(&src_dir).ok();
        fs::remove_dir_all(&out_dir).ok();
    }

    #[test]
    fn github_unsupported_for_now() {
        let src = Source::Github {
            repo: "acme/x".into(),
            git_ref: None,
            path: None,
        };
        assert!(matches!(fetch(&src), Err(FetchError::Unsupported(_))));
    }
}
