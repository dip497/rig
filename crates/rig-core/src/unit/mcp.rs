//! MCP server — canonical form covers stdio, HTTP, and SSE transports.
//!
//! See `docs/MCP-SUPPORT.md` for the full design. The canonical TOML
//! form emitted by [`canonical_toml`] is the SHA-significant input
//! used for drift detection.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mcp {
    pub name: String,
    /// Human-readable summary. NOT SHA-significant — the agent-native
    /// CLIs drop it, so Rig keeps it in the lockfile `extra` map, not
    /// in the canonical form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub transport: Transport,
    /// Declared env var names the server expects. Values are never
    /// embedded in the canonical unit — they come from shell env /
    /// agent-native secret stores. Hashed as a sorted, dedup'd vector.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
    /// Free-form hints (timeouts, auth kind, etc.). Adapters may
    /// ignore, but Rig still hashes the map so changes count as drift.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Transport {
    Stdio {
        command: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
    },
    Http {
        url: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        headers: BTreeMap<String, String>,
    },
    Sse {
        url: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        headers: BTreeMap<String, String>,
    },
}

impl Transport {
    /// Tag used in the canonical TOML form: `"stdio"`, `"http"`, or `"sse"`.
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Stdio { .. } => "stdio",
            Self::Http { .. } => "http",
            Self::Sse { .. } => "sse",
        }
    }
}

/// Deterministic TOML serialisation of an [`Mcp`] used as input to
/// `install_sha`. See `docs/MCP-SUPPORT.md` §2 for the rules:
///
/// - keys in a fixed order (`name, transport, command, args, url,
///   headers, env, metadata`);
/// - `transport` is the string tag, never a table;
/// - empty maps/vectors are omitted;
/// - `${VAR}` placeholders preserved byte-for-byte (no env expansion);
/// - `\n` line endings, no trailing whitespace, no BOM;
/// - `description` is **not** included.
#[must_use]
pub fn canonical_toml(mcp: &Mcp) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "name = {}", toml_string(&mcp.name));
    let _ = writeln!(out, "transport = {}", toml_string(mcp.transport.tag()));

    match &mcp.transport {
        Transport::Stdio { command, args } => {
            let _ = writeln!(out, "command = {}", toml_string(command));
            if !args.is_empty() {
                out.push_str("args = [");
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&toml_string(a));
                }
                out.push_str("]\n");
            }
        }
        Transport::Http { url, headers } | Transport::Sse { url, headers } => {
            let _ = writeln!(out, "url = {}", toml_string(url));
            if !headers.is_empty() {
                out.push_str("headers = { ");
                // `BTreeMap` already iterates in sorted-key order.
                for (i, (k, v)) in headers.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    let _ = write!(out, "{} = {}", toml_bare_key(k), toml_string(v));
                }
                out.push_str(" }\n");
            }
        }
    }

    if !mcp.env.is_empty() {
        let mut sorted: Vec<&String> = mcp.env.iter().collect();
        sorted.sort();
        sorted.dedup();
        out.push_str("env = [");
        for (i, e) in sorted.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&toml_string(e));
        }
        out.push_str("]\n");
    }

    if !mcp.metadata.is_empty() {
        out.push_str("metadata = { ");
        for (i, (k, v)) in mcp.metadata.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            let _ = write!(out, "{} = {}", toml_bare_key(k), toml_string(v));
        }
        out.push_str(" }\n");
    }

    out
}

/// Basic-string TOML escaping for values. Keeps things deterministic
/// and avoids pulling in a serialiser that might reorder or prettify.
fn toml_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Emit a bare key when safe (`[A-Za-z0-9_-]+`), otherwise quote it.
fn toml_bare_key(k: &str) -> String {
    let is_bare = !k.is_empty()
        && k.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if is_bare {
        k.to_owned()
    } else {
        toml_string(k)
    }
}

/// Parse the user-facing `mcp.toml` source surface described in
/// `docs/MCP-SUPPORT.md` §3 into a canonical [`Mcp`].
///
/// Strict parser: unknown top-level keys are rejected so typos surface
/// early. Supports the three transports (stdio, http, sse).
///
/// # Errors
/// Returns a string diagnostic when the TOML is malformed, the
/// `schema`/`kind` tags are wrong, or required fields are missing.
pub fn parse_source(s: &str) -> Result<Mcp, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Raw {
        schema: String,
        kind: String,
        name: String,
        #[serde(default)]
        description: Option<String>,
        transport: RawTransport,
        #[serde(default)]
        env: Vec<String>,
        #[serde(default)]
        metadata: BTreeMap<String, String>,
    }

    #[derive(Deserialize)]
    #[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
    enum RawTransport {
        Stdio {
            command: String,
            #[serde(default)]
            args: Vec<String>,
        },
        Http {
            url: String,
            #[serde(default)]
            headers: BTreeMap<String, String>,
        },
        Sse {
            url: String,
            #[serde(default)]
            headers: BTreeMap<String, String>,
        },
    }

    let raw: Raw = toml::from_str(s).map_err(|e| format!("invalid mcp.toml: {e}"))?;
    if raw.schema != "rig/v1" {
        return Err(format!(
            "unsupported mcp.toml schema `{}` (expected `rig/v1`)",
            raw.schema
        ));
    }
    if raw.kind != "mcp" {
        return Err(format!("mcp.toml kind must be `mcp`, got `{}`", raw.kind));
    }

    let transport = match raw.transport {
        RawTransport::Stdio { command, args } => Transport::Stdio { command, args },
        RawTransport::Http { url, headers } => Transport::Http { url, headers },
        RawTransport::Sse { url, headers } => Transport::Sse { url, headers },
    };

    Ok(Mcp {
        name: raw.name,
        description: raw.description,
        transport,
        env: raw.env,
        metadata: raw.metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m_stdio() -> Mcp {
        Mcp {
            name: "github".into(),
            description: Some("GitHub MCP".into()),
            transport: Transport::Stdio {
                command: "npx".into(),
                args: vec!["-y".into(), "@modelcontextprotocol/server-github".into()],
            },
            env: vec!["GITHUB_TOKEN".into()],
            metadata: BTreeMap::new(),
        }
    }

    fn m_http() -> Mcp {
        let mut headers = BTreeMap::new();
        headers.insert("Authorization".into(), "Bearer ${FIGMA_TOKEN}".into());
        Mcp {
            name: "figma".into(),
            description: None,
            transport: Transport::Http {
                url: "https://mcp.figma.com/".into(),
                headers,
            },
            env: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    fn m_sse() -> Mcp {
        Mcp {
            name: "analytics".into(),
            description: None,
            transport: Transport::Sse {
                url: "https://analytics.example.com/sse".into(),
                headers: BTreeMap::new(),
            },
            env: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn canonical_stdio_golden() {
        let got = canonical_toml(&m_stdio());
        let want = "name = \"github\"\n\
                    transport = \"stdio\"\n\
                    command = \"npx\"\n\
                    args = [\"-y\", \"@modelcontextprotocol/server-github\"]\n\
                    env = [\"GITHUB_TOKEN\"]\n";
        assert_eq!(got, want);
    }

    #[test]
    fn canonical_http_golden_preserves_placeholder() {
        let got = canonical_toml(&m_http());
        let want = "name = \"figma\"\n\
                    transport = \"http\"\n\
                    url = \"https://mcp.figma.com/\"\n\
                    headers = { Authorization = \"Bearer ${FIGMA_TOKEN}\" }\n";
        assert_eq!(got, want);
    }

    #[test]
    fn canonical_sse_omits_empty_headers() {
        let got = canonical_toml(&m_sse());
        let want = "name = \"analytics\"\n\
                    transport = \"sse\"\n\
                    url = \"https://analytics.example.com/sse\"\n";
        assert_eq!(got, want);
    }

    #[test]
    fn description_not_sha_significant() {
        let mut a = m_stdio();
        let mut b = m_stdio();
        a.description = Some("aaa".into());
        b.description = Some("zzz".into());
        assert_eq!(canonical_toml(&a), canonical_toml(&b));
    }

    #[test]
    fn env_sorted_and_deduped() {
        let mut m = m_stdio();
        m.env = vec!["B".into(), "A".into(), "A".into()];
        let t = canonical_toml(&m);
        assert!(t.contains("env = [\"A\", \"B\"]"));
    }

    #[test]
    fn parse_source_roundtrip_stdio() {
        let toml = r#"schema = "rig/v1"
kind = "mcp"
name = "github"
description = "GitHub MCP"
env = ["GITHUB_TOKEN"]

[transport]
kind = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
"#;
        let m = parse_source(toml).unwrap();
        assert_eq!(m.name, "github");
        assert_eq!(m.description.as_deref(), Some("GitHub MCP"));
        assert!(matches!(m.transport, Transport::Stdio { .. }));
    }

    #[test]
    fn parse_source_http_headers() {
        let toml = r#"schema = "rig/v1"
kind = "mcp"
name = "figma"

[transport]
kind = "http"
url = "https://mcp.figma.com/"
headers = { Authorization = "Bearer ${FIGMA_TOKEN}" }
"#;
        let m = parse_source(toml).unwrap();
        match m.transport {
            Transport::Http { url, headers } => {
                assert_eq!(url, "https://mcp.figma.com/");
                assert_eq!(
                    headers.get("Authorization").map(String::as_str),
                    Some("Bearer ${FIGMA_TOKEN}")
                );
            }
            _ => panic!("expected Http"),
        }
    }

    #[test]
    fn parse_source_rejects_unknown_keys() {
        let toml = r#"schema = "rig/v1"
kind = "mcp"
name = "x"
bogus = "nope"

[transport]
kind = "stdio"
command = "echo"
"#;
        assert!(parse_source(toml).is_err());
    }

    #[test]
    fn parse_source_rejects_bad_schema() {
        let toml = r#"schema = "rig/v99"
kind = "mcp"
name = "x"

[transport]
kind = "stdio"
command = "echo"
"#;
        assert!(parse_source(toml).is_err());
    }
}
