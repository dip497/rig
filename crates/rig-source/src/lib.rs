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

use std::io::Read;
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
    #[error("http error: {0}")]
    Http(String),
    #[error("git error: {0}")]
    Git(String),
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
        Source::Http { url } => fetch_http(source, url),
        Source::Github {
            repo,
            git_ref,
            path,
        } => fetch_github(source, repo, git_ref.as_deref(), path.as_deref()),
        Source::Git { .. } | Source::Npm { .. } | Source::Marketplace { .. } => {
            Err(FetchError::Unsupported(source.to_string()))
        }
    }
}

/// Build the cache directory path for a resolved github repo at SHA.
pub fn github_cache_dir(owner: &str, repo: &str, sha: &str) -> FetchResult<PathBuf> {
    let home = rig_fs::home_dir().map_err(|e| FetchError::Git(e.to_string()))?;
    Ok(home
        .join(".rig")
        .join("cache")
        .join("github")
        .join(owner)
        .join(format!("{repo}@{sha}")))
}

/// Build the HTTPS clone URL for a `owner/repo` string.
#[must_use]
pub fn github_clone_url(repo: &str) -> String {
    format!("https://github.com/{repo}.git")
}

fn fetch_github(
    source: &Source,
    repo: &str,
    git_ref: Option<&str>,
    subpath: Option<&str>,
) -> FetchResult<Fetched> {
    let (owner, name) = repo
        .split_once('/')
        .ok_or_else(|| FetchError::Git(format!("bad repo `{repo}` (expected owner/name)")))?;
    let url = github_clone_url(repo);
    let ref_spec = git_ref.unwrap_or("HEAD");

    // Resolve ref → SHA via `git ls-remote`.
    let sha = resolve_ref(&url, ref_spec)?;
    let cache = github_cache_dir(owner, name, &sha)?;

    // If the SHA isn't cached yet, shallow-clone into it.
    if !cache.join(".git").exists() {
        if let Some(parent) = cache.parent() {
            std::fs::create_dir_all(parent).map_err(|e| FetchError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        std::fs::create_dir_all(&cache).map_err(|e| FetchError::Io {
            path: cache.clone(),
            source: e,
        })?;
        run_git(&cache, &["init", "-q"])?;
        run_git(&cache, &["remote", "add", "origin", &url])?;
        run_git(&cache, &["fetch", "--depth=1", "-q", "origin", &sha])?;
        run_git(&cache, &["checkout", "-q", &sha])?;
    }

    let root = match subpath {
        Some(p) => cache.join(p),
        None => cache.clone(),
    };
    if !root.exists() {
        return Err(FetchError::Git(format!(
            "path `{}` not found in {repo}@{sha}",
            root.display()
        )));
    }

    let (walk_root, detected) = detect(&root)?;
    let files = if root.is_file() {
        vec![root.clone()]
    } else {
        collect(&walk_root).map_err(|e| FetchError::Io {
            path: walk_root.clone(),
            source: e,
        })?
    };
    let (native, hash_input) = read_files(&files, &walk_root, root.is_file())?;

    Ok(Fetched {
        source: source.clone(),
        source_sha: Sha256::of(&hash_input),
        native: NativeLayout { files: native },
        detected,
    })
}

fn resolve_ref(url: &str, ref_spec: &str) -> FetchResult<String> {
    let output = std::process::Command::new("git")
        .args(["ls-remote", url, ref_spec])
        .output()
        .map_err(|e| FetchError::Git(format!("invoking git: {e}")))?;
    if !output.status.success() {
        return Err(FetchError::Git(format!(
            "git ls-remote {url} {ref_spec}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    // First line, first column.
    let line = text
        .lines()
        .next()
        .ok_or_else(|| FetchError::Git(format!("ref `{ref_spec}` not found in {url}")))?;
    let sha = line
        .split_whitespace()
        .next()
        .ok_or_else(|| FetchError::Git(format!("malformed ls-remote output for {url}")))?;
    if sha.len() < 7 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(FetchError::Git(format!(
            "unexpected sha `{sha}` from ls-remote"
        )));
    }
    Ok(sha.to_owned())
}

fn run_git(cwd: &Path, args: &[&str]) -> FetchResult<()> {
    let output = std::process::Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .map_err(|e| FetchError::Git(format!("invoking git {args:?}: {e}")))?;
    if !output.status.success() {
        return Err(FetchError::Git(format!(
            "git {args:?} in {}: {}",
            cwd.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

fn fetch_http(source: &Source, url: &str) -> FetchResult<Fetched> {
    let resp = ureq::get(url)
        .call()
        .map_err(|e| FetchError::Http(format!("GET {url}: {e}")))?;

    let mut bytes: Vec<u8> = Vec::new();
    resp.into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| FetchError::Http(format!("reading body from {url}: {e}")))?;

    let source_sha = Sha256::of(&bytes);

    // Strip query string / fragment when determining file type.
    let clean = url.split(['?', '#']).next().unwrap_or(url);
    let lower = clean.to_ascii_lowercase();

    let is_tar = lower.ends_with(".rig") || lower.ends_with(".tar.gz") || lower.ends_with(".tgz");
    let is_md = lower.ends_with(".md");

    if is_tar {
        // Write to temp, then delegate to tarball logic.
        let tmp = tempfile::Builder::new()
            .prefix("rig-http-")
            .suffix(if lower.ends_with(".tgz") {
                ".tgz"
            } else if lower.ends_with(".tar.gz") {
                ".tar.gz"
            } else {
                ".rig"
            })
            .tempfile()
            .map_err(|e| FetchError::Io {
                path: PathBuf::from(url),
                source: e,
            })?;
        std::fs::write(tmp.path(), &bytes).map_err(|e| FetchError::Io {
            path: tmp.path().to_path_buf(),
            source: e,
        })?;
        let temp = rig_fs::unpack_to_temp(tmp.path()).map_err(|e| FetchError::Io {
            path: tmp.path().to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
        })?;
        let (root, detected) = detect(temp.path())?;
        let files = collect(&root).map_err(|s| FetchError::Io {
            path: root.clone(),
            source: s,
        })?;
        let (native, _) = read_files(&files, &root, false)?;
        return Ok(Fetched {
            source: source.clone(),
            source_sha,
            native: NativeLayout { files: native },
            detected,
        });
    }

    if is_md {
        let filename = clean
            .rsplit('/')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("file.md")
            .to_owned();
        let detected = if filename == "SKILL.md" {
            Some(UnitType::Skill)
        } else {
            None
        };
        let mut hash_input = Vec::new();
        hash_input.extend_from_slice(filename.as_bytes());
        hash_input.push(0);
        hash_input.extend_from_slice(&bytes);
        hash_input.push(0);
        // NB: source_sha is over raw response bytes (deterministic). We
        // keep the response-byte sha — not the rel+bytes encoding.
        return Ok(Fetched {
            source: source.clone(),
            source_sha,
            native: NativeLayout {
                files: vec![NativeFile {
                    relative_path: filename,
                    bytes,
                }],
            },
            detected,
        });
    }

    Err(FetchError::Undetected(PathBuf::from(url)))
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

    // `mcp.toml` at the root (or passed directly as the file) → Mcp.
    // Per `docs/MCP-SUPPORT.md` §3, the MCP source format is a single
    // `mcp.toml` file. Adapter-side `parse_native` consumes it.
    if root.join("mcp.toml").exists() || file_hint.as_deref() == Some("mcp.toml") {
        return Ok((root, Some(UnitType::Mcp)));
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
    fn fetches_local_mcp_dir() {
        let dir = tempdir("mcp");
        fs::write(
            dir.join("mcp.toml"),
            "schema = \"rig/v1\"\nkind = \"mcp\"\nname = \"demo\"\n\n[transport]\nkind = \"stdio\"\ncommand = \"echo\"\n",
        )
        .unwrap();
        let src = Source::Local {
            path: dir.to_string_lossy().into_owned(),
        };
        let f = fetch(&src).unwrap();
        assert_eq!(f.detected, Some(UnitType::Mcp));
        assert!(f.native.files.iter().any(|f| f.relative_path == "mcp.toml"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fetches_direct_mcp_toml_file() {
        let dir = tempdir("mcp-file");
        let p = dir.join("mcp.toml");
        fs::write(
            &p,
            "schema = \"rig/v1\"\nkind = \"mcp\"\nname = \"demo\"\n\n[transport]\nkind = \"http\"\nurl = \"https://x\"\n",
        )
        .unwrap();
        let src = Source::Local {
            path: p.to_string_lossy().into_owned(),
        };
        let f = fetch(&src).unwrap();
        assert_eq!(f.detected, Some(UnitType::Mcp));
        assert_eq!(f.native.files.len(), 1);
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
    fn github_clone_url_format() {
        assert_eq!(
            github_clone_url("anthropics/claude-code"),
            "https://github.com/anthropics/claude-code.git"
        );
    }

    #[test]
    fn github_cache_dir_shape() {
        let p = github_cache_dir("owner", "repo", "abc123").unwrap();
        let s = p.to_string_lossy();
        assert!(s.contains(".rig/cache/github/owner/repo@abc123"));
    }

    #[test]
    fn http_tarball_fetch() {
        let src_dir = tempdir("http-tb-src");
        fs::write(
            src_dir.join("SKILL.md"),
            "---\nname: http-skill\ndescription: d\n---\nbody\n",
        )
        .unwrap();
        let out_dir = tempdir("http-tb-out");
        let archive = out_dir.join("demo.rig");
        rig_fs::pack_dir(&src_dir, &archive).unwrap();
        let bytes = fs::read(&archive).unwrap();

        let mut server = mockito::Server::new();
        let m = server
            .mock("GET", "/demo.rig")
            .with_status(200)
            .with_header("content-type", "application/octet-stream")
            .with_body(bytes.clone())
            .expect(2)
            .create();

        let url = format!("{}/demo.rig", server.url());
        let src = Source::parse(&url).unwrap();
        let f = fetch(&src).unwrap();
        assert_eq!(f.detected, Some(UnitType::Skill));
        assert!(f.native.files.iter().any(|f| f.relative_path == "SKILL.md"));
        // SHA of raw bytes deterministic.
        let f2 = fetch(&src).unwrap();
        assert_eq!(f.source_sha, f2.source_sha);

        m.assert();
        fs::remove_dir_all(&src_dir).ok();
        fs::remove_dir_all(&out_dir).ok();
    }

    #[test]
    fn http_md_single_file() {
        let body = b"---\nname: my-rule\n---\nbody\n";
        let mut server = mockito::Server::new();
        let m = server
            .mock("GET", "/rules/my-rule.md")
            .with_status(200)
            .with_body(body)
            .create();
        let url = format!("{}/rules/my-rule.md", server.url());
        let src = Source::parse(&url).unwrap();
        let f = fetch(&src).unwrap();
        assert_eq!(f.native.files.len(), 1);
        assert_eq!(f.native.files[0].relative_path, "my-rule.md");
        assert!(f.detected.is_none());
        m.assert();
    }

    #[test]
    fn http_404_errors() {
        let mut server = mockito::Server::new();
        let _m = server.mock("GET", "/missing.rig").with_status(404).create();
        let url = format!("{}/missing.rig", server.url());
        let src = Source::parse(&url).unwrap();
        let r = fetch(&src);
        assert!(matches!(r, Err(FetchError::Http(_))));
    }
}
