//! Where a unit comes from, and content-addressed SHAs.
//!
//! Sources are parsed from manifest strings like `github:owner/repo@v1.2`
//! or `local:./skills/foo`. Actual fetching lives in `rig-source`.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Source {
    Github {
        repo: String,
        #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
        git_ref: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    Git {
        url: String,
        #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
        git_ref: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    Npm {
        package: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,
    },
    Local {
        path: String,
    },
    Marketplace {
        id: String,
    },
}

impl Source {
    /// Parse the short-form string used in `rig.toml` bundles.
    ///
    /// Examples:
    /// - `github:acme/react-review@v1.2`
    /// - `github:acme/react-review@v1.2#skills/foo`
    /// - `local:./skills/foo`
    /// - `npm:@scope/pkg@1.0`
    /// - `marketplace:figma-mcp`
    ///
    /// # Errors
    /// Returns [`SourceParseError`] when the scheme is unknown or the
    /// body is malformed for the declared scheme.
    pub fn parse(s: &str) -> Result<Self, SourceParseError> {
        // Bare filesystem paths → implicit `local:` scheme.
        if s.starts_with("./") || s.starts_with("../") || s.starts_with('/') || s.starts_with("~/")
        {
            return Ok(Self::Local { path: s.to_owned() });
        }

        let (scheme, rest) = s
            .split_once(':')
            .ok_or_else(|| SourceParseError::MissingScheme(s.to_owned()))?;

        match scheme {
            "github" => {
                let (repo_ref, path) = split_path(rest);
                let (repo, git_ref) = split_ref(repo_ref);
                if !repo.contains('/') {
                    return Err(SourceParseError::BadGithub(s.to_owned()));
                }
                Ok(Self::Github {
                    repo: repo.to_owned(),
                    git_ref: git_ref.map(str::to_owned),
                    path,
                })
            }
            "git" => {
                let (url_ref, path) = split_path(rest);
                let (url, git_ref) = split_ref(url_ref);
                Ok(Self::Git {
                    url: url.to_owned(),
                    git_ref: git_ref.map(str::to_owned),
                    path,
                })
            }
            "npm" => {
                let (package, version) = split_npm(rest);
                Ok(Self::Npm {
                    package: package.to_owned(),
                    version: version.map(str::to_owned),
                })
            }
            "local" => Ok(Self::Local {
                path: rest.to_owned(),
            }),
            "marketplace" => Ok(Self::Marketplace {
                id: rest.to_owned(),
            }),
            other => Err(SourceParseError::UnknownScheme(other.to_owned())),
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Github {
                repo,
                git_ref,
                path,
            } => {
                write!(f, "github:{repo}")?;
                if let Some(r) = git_ref {
                    write!(f, "@{r}")?;
                }
                if let Some(p) = path {
                    write!(f, "#{p}")?;
                }
                Ok(())
            }
            Self::Git { url, git_ref, path } => {
                write!(f, "git:{url}")?;
                if let Some(r) = git_ref {
                    write!(f, "@{r}")?;
                }
                if let Some(p) = path {
                    write!(f, "#{p}")?;
                }
                Ok(())
            }
            Self::Npm { package, version } => {
                write!(f, "npm:{package}")?;
                if let Some(v) = version {
                    write!(f, "@{v}")?;
                }
                Ok(())
            }
            Self::Local { path } => write!(f, "local:{path}"),
            Self::Marketplace { id } => write!(f, "marketplace:{id}"),
        }
    }
}

fn split_path(s: &str) -> (&str, Option<String>) {
    match s.split_once('#') {
        Some((a, b)) => (a, Some(b.to_owned())),
        None => (s, None),
    }
}

fn split_ref(s: &str) -> (&str, Option<&str>) {
    match s.rsplit_once('@') {
        Some((a, b)) => (a, Some(b)),
        None => (s, None),
    }
}

fn split_npm(s: &str) -> (&str, Option<&str>) {
    // `@scope/pkg@ver` — version is after the LAST `@`, and only if the
    // remainder before it still contains the package name.
    if let Some(at) = s.rfind('@') {
        if at > 0 {
            return (&s[..at], Some(&s[at + 1..]));
        }
    }
    (s, None)
}

#[derive(Debug, thiserror::Error)]
pub enum SourceParseError {
    #[error("source `{0}` missing scheme (expected e.g. `github:owner/repo`)")]
    MissingScheme(String),
    #[error("unknown source scheme `{0}`")]
    UnknownScheme(String),
    #[error("github source `{0}` must be `github:owner/repo[@ref][#path]`")]
    BadGithub(String),
}

/// Content-addressed SHA-256 digest, 64 lowercase hex chars.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Sha256(String);

impl Sha256 {
    /// Hash arbitrary bytes.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        use sha2::{Digest, Sha256 as Hasher};
        let digest = Hasher::digest(bytes);
        Self(hex_encode(&digest))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_full() {
        let s = Source::parse("github:acme/react-review@v1.2#skills/foo").unwrap();
        assert_eq!(
            s,
            Source::Github {
                repo: "acme/react-review".into(),
                git_ref: Some("v1.2".into()),
                path: Some("skills/foo".into()),
            }
        );
    }

    #[test]
    fn parses_github_bare() {
        let s = Source::parse("github:acme/react-review").unwrap();
        assert_eq!(
            s,
            Source::Github {
                repo: "acme/react-review".into(),
                git_ref: None,
                path: None,
            }
        );
    }

    #[test]
    fn parses_local() {
        assert_eq!(
            Source::parse("local:./foo").unwrap(),
            Source::Local {
                path: "./foo".into()
            }
        );
    }

    #[test]
    fn parses_npm_scoped() {
        assert_eq!(
            Source::parse("npm:@scope/pkg@1.0").unwrap(),
            Source::Npm {
                package: "@scope/pkg".into(),
                version: Some("1.0".into()),
            }
        );
    }

    #[test]
    fn roundtrip_display() {
        let raw = "github:acme/x@v1#p";
        assert_eq!(Source::parse(raw).unwrap().to_string(), raw);
    }

    #[test]
    fn rejects_bad_github() {
        assert!(Source::parse("github:noSlash").is_err());
    }

    #[test]
    fn rejects_missing_scheme() {
        assert!(Source::parse("acme/foo").is_err());
    }

    #[test]
    fn sha256_is_deterministic() {
        let a = Sha256::of(b"hello");
        let b = Sha256::of(b"hello");
        assert_eq!(a, b);
        assert_eq!(a.as_str().len(), 64);
    }
}
