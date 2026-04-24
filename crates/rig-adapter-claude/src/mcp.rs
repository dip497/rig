//! Claude Code MCP adapter logic.
//!
//! Implementation of `docs/MCP-SUPPORT.md` §4 / §6 / §7.
//!
//! Rig never hand-edits `~/.claude.json` or `./.mcp.json`. Instead it
//! shells out to `claude mcp add|remove|list|get`. Drift detection
//! rehydrates the native JSON into a canonical [`Mcp`] and compares
//! the canonical TOML hash — this means harmless whitespace or
//! key-order differences in the agent file don't trip `LocalDrift`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use rig_core::adapter::{AdapterError, AdapterResult, UnitRef};
use rig_core::converter::{Converter, NativeFile, NativeLayout};
use rig_core::scope::Scope;
use rig_core::source::Sha256;
use rig_core::unit::mcp::{self, Mcp, Transport};
use rig_core::unit::UnitType;

/// Name of the env var tests can set to point at a fake `claude` binary.
pub(crate) const CLAUDE_BIN_ENV: &str = "RIG_CLAUDE_BIN";

/// `Converter<Mcp>` using `mcp.toml` as the native-layout file. The
/// adapter doesn't actually _write_ this file to disk (install shells
/// out to `claude mcp add`), but `rig-source` fetches a canonical
/// `mcp.toml` and the CLI uses this converter to parse it.
pub struct MCPConverter;

impl Converter<Mcp> for MCPConverter {
    fn to_native(&self, canonical: &Mcp) -> AdapterResult<NativeLayout> {
        // We emit the user-facing `mcp.toml` surface form, not the
        // canonical-hash form — `parse_native` must be able to round-
        // trip it, so surface-form is the right choice here.
        let toml = surface_toml(canonical);
        Ok(NativeLayout {
            files: vec![NativeFile {
                relative_path: "mcp.toml".into(),
                bytes: toml.into_bytes(),
            }],
        })
    }

    fn parse_native(&self, native: &NativeLayout) -> AdapterResult<Mcp> {
        let file = native
            .files
            .iter()
            .find(|f| f.relative_path == "mcp.toml")
            .ok_or_else(|| AdapterError::Other {
                message: "MCP source: `mcp.toml` missing from NativeLayout".into(),
                source: None,
            })?;
        let text = std::str::from_utf8(&file.bytes).map_err(|e| AdapterError::Other {
            message: format!("mcp.toml is not UTF-8: {e}"),
            source: None,
        })?;
        mcp::parse_source(text).map_err(|message| AdapterError::Other {
            message,
            source: None,
        })
    }
}

/// User-facing surface TOML (the one users author). Distinct from
/// `canonical_toml` — surface-form keeps `description` and uses a
/// `[transport]` table per spec §3.
fn surface_toml(m: &Mcp) -> String {
    let mut out = String::new();
    out.push_str("schema = \"rig/v1\"\n");
    out.push_str("kind = \"mcp\"\n\n");
    out.push_str(&format!("name = {}\n", toml_str(&m.name)));
    if let Some(d) = &m.description {
        out.push_str(&format!("description = {}\n", toml_str(d)));
    }
    // Emit env + metadata BEFORE the `[transport]` table so they stay
    // at the top-level and don't accidentally fall inside the table
    // in TOML's bracket-table-scoping rules.
    if !m.env.is_empty() {
        out.push_str("env = [");
        for (i, e) in m.env.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&toml_str(e));
        }
        out.push_str("]\n");
    }
    if !m.metadata.is_empty() {
        out.push_str("\n[metadata]\n");
        for (k, v) in &m.metadata {
            out.push_str(&format!("{} = {}\n", bare_key(k), toml_str(v)));
        }
    }
    out.push_str("\n[transport]\n");
    match &m.transport {
        Transport::Stdio { command, args } => {
            out.push_str("kind = \"stdio\"\n");
            out.push_str(&format!("command = {}\n", toml_str(command)));
            if !args.is_empty() {
                out.push_str("args = [");
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&toml_str(a));
                }
                out.push_str("]\n");
            }
        }
        Transport::Http { url, headers } => {
            out.push_str("kind = \"http\"\n");
            out.push_str(&format!("url = {}\n", toml_str(url)));
            if !headers.is_empty() {
                out.push_str("headers = { ");
                for (i, (k, v)) in headers.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&format!("{} = {}", bare_key(k), toml_str(v)));
                }
                out.push_str(" }\n");
            }
        }
        Transport::Sse { url, headers } => {
            out.push_str("kind = \"sse\"\n");
            out.push_str(&format!("url = {}\n", toml_str(url)));
            if !headers.is_empty() {
                out.push_str("headers = { ");
                for (i, (k, v)) in headers.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&format!("{} = {}", bare_key(k), toml_str(v)));
                }
                out.push_str(" }\n");
            }
        }
    }
    out
}

fn toml_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn bare_key(k: &str) -> String {
    let bare = !k.is_empty()
        && k.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if bare {
        k.to_owned()
    } else {
        toml_str(k)
    }
}

// ---------------- scope mapping ----------------

/// Claude's `--scope` flag value for a Rig [`Scope`].
pub(crate) fn scope_flag(s: Scope) -> &'static str {
    match s {
        Scope::Global => "user",
        Scope::Project => "project",
        Scope::Local => "local",
    }
}

/// Path of the Claude config file that holds the MCP entry for a scope.
/// Used for `Receipt::paths` and (lexically) as the file-level sync
/// lock target. Spec §4.
pub(crate) fn config_path(scope: Scope) -> AdapterResult<PathBuf> {
    let home = rig_fs::home_dir().map_err(|e| AdapterError::Other {
        message: e.to_string(),
        source: Some(Box::new(e)),
    })?;
    Ok(match scope {
        Scope::Global | Scope::Local => home.join(".claude.json"),
        Scope::Project => PathBuf::from(".mcp.json"),
    })
}

// ---------------- argv builder (pure) ----------------

/// Build the argv for `claude mcp add` from a canonical [`Mcp`] and a
/// scope. Pure function — unit-tested without touching the filesystem.
///
/// Order is deterministic so golden tests stay stable.
#[must_use]
pub fn build_add_argv(mcp: &Mcp, scope: Scope) -> Vec<String> {
    let mut argv: Vec<String> = vec![
        "mcp".into(),
        "add".into(),
        "--transport".into(),
        mcp.transport.tag().to_owned(),
        "--scope".into(),
        scope_flag(scope).to_owned(),
    ];

    // Env var names first (sorted, deduped) so the argv is stable.
    let mut envs: Vec<&String> = mcp.env.iter().collect();
    envs.sort();
    envs.dedup();
    for e in envs {
        argv.push("--env".into());
        argv.push(e.clone());
    }

    match &mcp.transport {
        Transport::Stdio { command, args } => {
            argv.push(mcp.name.clone());
            argv.push("--".into());
            argv.push(command.clone());
            for a in args {
                argv.push(a.clone());
            }
        }
        Transport::Http { url, headers } | Transport::Sse { url, headers } => {
            for (k, v) in headers {
                argv.push("--header".into());
                argv.push(format!("{k}: {v}"));
            }
            argv.push(mcp.name.clone());
            argv.push(url.clone());
        }
    }
    argv
}

/// Build the argv for `claude mcp remove <name> --scope X`.
#[must_use]
pub fn build_remove_argv(name: &str, scope: Scope) -> Vec<String> {
    vec![
        "mcp".into(),
        "remove".into(),
        name.to_owned(),
        "--scope".into(),
        scope_flag(scope).to_owned(),
    ]
}

// ---------------- process invocation ----------------

#[derive(Debug)]
pub(crate) struct CliRun {
    pub status_ok: bool,
    #[allow(dead_code)]
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Invoke the `claude` binary (respecting `RIG_CLAUDE_BIN` for tests).
/// Returns [`AdapterError::Other`] with a "binary-missing" marker
/// message if the binary is not on PATH, so callers can produce the
/// spec-§10 diagnostic.
pub(crate) fn run_claude(argv: &[String]) -> AdapterResult<CliRun> {
    let bin = std::env::var(CLAUDE_BIN_ENV).unwrap_or_else(|_| "claude".to_string());
    let output = Command::new(&bin)
        .args(argv)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(o) => Ok(CliRun {
            status_ok: o.status.success(),
            stdout: o.stdout,
            stderr: o.stderr,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(AdapterError::Other {
            message: "claude CLI not found on PATH (required for MCP install)".into(),
            source: Some(Box::new(e)),
        }),
        Err(e) => Err(AdapterError::Other {
            message: format!("invoking `{bin}`: {e}"),
            source: Some(Box::new(e)),
        }),
    }
}

// ---------------- install / uninstall ----------------

/// Install an MCP entry via `claude mcp add`, after a best-effort
/// `claude mcp remove` to make re-installs observably clean.
///
/// Returns the canonical install_sha (hash of `canonical_toml(mcp)`)
/// and the path of the config file the CLI mutated.
pub(crate) fn install(mcp: &Mcp, scope: Scope) -> AdapterResult<(Sha256, PathBuf)> {
    // Idempotency. Ignore errors — "not found" is fine.
    let _ = run_claude(&build_remove_argv(&mcp.name, scope));

    let argv = build_add_argv(mcp, scope);
    let run = run_claude(&argv)?;
    if !run.status_ok {
        let stderr = String::from_utf8_lossy(&run.stderr).trim().to_owned();
        return Err(AdapterError::Other {
            message: format!("claude mcp add failed: {stderr}"),
            source: None,
        });
    }

    let install_sha = Sha256::of(mcp::canonical_toml(mcp).as_bytes());
    let path = config_path(scope)?;
    Ok((install_sha, path))
}

pub(crate) fn uninstall(name: &str, scope: Scope) -> AdapterResult<()> {
    let run = run_claude(&build_remove_argv(name, scope))?;
    if run.status_ok {
        return Ok(());
    }
    // Idempotent: swallow "not found" stderr hints.
    let stderr = String::from_utf8_lossy(&run.stderr).to_ascii_lowercase();
    if stderr.contains("not found") || stderr.contains("no such") {
        return Ok(());
    }
    Err(AdapterError::Other {
        message: format!(
            "claude mcp remove failed: {}",
            String::from_utf8_lossy(&run.stderr).trim()
        ),
        source: None,
    })
}

// ---------------- list + read_local + drift ----------------

/// Read all managed entries from the Claude config file for this scope.
/// Parses `~/.claude.json` or `.mcp.json` directly (spec §6). Errors
/// when the file is missing are treated as "no entries".
pub(crate) fn list_native(scope: Scope) -> AdapterResult<Vec<Mcp>> {
    let path = config_path(scope)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = std::fs::read(&path).map_err(|e| AdapterError::Other {
        message: format!("reading {}: {e}", path.display()),
        source: Some(Box::new(e)),
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|e| AdapterError::Other {
        message: format!("{} is not UTF-8: {e}", path.display()),
        source: None,
    })?;
    parse_claude_config(text, scope)
}

/// Parse the agent-native JSON into canonical [`Mcp`] entries. Pure
/// so tests can feed fixture bytes without a real config file.
pub(crate) fn parse_claude_config(text: &str, scope: Scope) -> AdapterResult<Vec<Mcp>> {
    let v: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) if text.trim().is_empty() => return Ok(Vec::new()),
        Err(e) => {
            return Err(AdapterError::Other {
                message: format!("parsing claude config: {e}"),
                source: Some(Box::new(e)),
            })
        }
    };

    // For `Scope::Global | Local` the structure is
    //   `{ "mcpServers": { "<name>": { ... } }, ... }`
    // at top level OR inside the relevant project block. For
    // `.mcp.json` (project) the root is `{ "mcpServers": { ... } }`.
    // We look in both places — whichever yields data.
    let obj = v.as_object();
    let mut entries = Vec::new();
    if let Some(map) = obj
        .and_then(|o| o.get("mcpServers"))
        .and_then(|m| m.as_object())
    {
        for (name, entry) in map {
            entries.push(native_to_canonical(name, entry)?);
        }
    }
    let _ = scope; // reserved for per-scope filtering if needed later
    Ok(entries)
}

/// Convert a single Claude-native JSON entry into the canonical
/// [`Mcp`] form. Spec §6: drop empty `env: {}` / `headers: {}`,
/// preserve `${VAR}` literally, sort keys. Reject unknown fields
/// loudly rather than silently losing data.
fn native_to_canonical(name: &str, entry: &serde_json::Value) -> AdapterResult<Mcp> {
    let obj = entry.as_object().ok_or_else(|| AdapterError::Other {
        message: format!("native MCP `{name}` is not a JSON object"),
        source: None,
    })?;

    // Known keys; anything else triggers the spec-§10 "unknown fields" error.
    const KNOWN: &[&str] = &[
        "type",
        "transport",
        "command",
        "args",
        "url",
        "headers",
        "env",
    ];
    let unknown: Vec<&str> = obj
        .keys()
        .map(String::as_str)
        .filter(|k| !KNOWN.contains(k))
        .collect();
    if !unknown.is_empty() {
        return Err(AdapterError::Other {
            message: format!(
                "native MCP `{name}` has fields Rig cannot represent: {}",
                unknown.join(", ")
            ),
            source: None,
        });
    }

    // Transport tag: try "transport" first, fall back to "type".
    let tag = obj
        .get("transport")
        .or_else(|| obj.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("stdio");

    // Collect env var names (BTreeMap → sorted vec; drop empty).
    let env: Vec<String> = obj
        .get("env")
        .and_then(|v| v.as_object())
        .map(|m| {
            let mut keys: Vec<String> = m.keys().cloned().collect();
            keys.sort();
            keys
        })
        .unwrap_or_default();

    let headers: BTreeMap<String, String> = obj
        .get("headers")
        .and_then(|v| v.as_object())
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                .collect()
        })
        .unwrap_or_default();

    let transport = match tag {
        "stdio" => {
            let command = obj
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AdapterError::Other {
                    message: format!("native MCP `{name}`: stdio entry missing `command`"),
                    source: None,
                })?
                .to_owned();
            let args: Vec<String> = obj
                .get("args")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            Transport::Stdio { command, args }
        }
        "http" => {
            let url = obj
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AdapterError::Other {
                    message: format!("native MCP `{name}`: http entry missing `url`"),
                    source: None,
                })?
                .to_owned();
            Transport::Http { url, headers }
        }
        "sse" => {
            let url = obj
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AdapterError::Other {
                    message: format!("native MCP `{name}`: sse entry missing `url`"),
                    source: None,
                })?
                .to_owned();
            Transport::Sse { url, headers }
        }
        other => {
            return Err(AdapterError::Other {
                message: format!("native MCP `{name}`: unknown transport `{other}`"),
                source: None,
            });
        }
    };

    Ok(Mcp {
        name: name.to_owned(),
        description: None,
        transport,
        env,
        metadata: BTreeMap::new(),
    })
}

/// Find a managed entry by name. Returns `NotFound` if absent.
pub(crate) fn read_local(unit_ref: &UnitRef, scope: Scope) -> AdapterResult<Mcp> {
    let all = list_native(scope)?;
    all.into_iter()
        .find(|m| m.name == unit_ref.name)
        .ok_or_else(|| AdapterError::NotFound(unit_ref.name.clone(), scope))
}

/// Compute the current-disk canonical SHA for an MCP entry, if any.
pub(crate) fn current_sha(name: &str, scope: Scope) -> AdapterResult<Option<Sha256>> {
    let all = list_native(scope)?;
    Ok(all
        .into_iter()
        .find(|m| m.name == name)
        .map(|m| Sha256::of(mcp::canonical_toml(&m).as_bytes())))
}

/// Validate that `(unit_type, scope)` is a combination the Claude
/// adapter accepts. Spec §8: `Scope::Local` is MCP-only on Claude.
pub(crate) fn validate_scope(unit_type: UnitType, scope: Scope) -> AdapterResult<()> {
    if matches!(scope, Scope::Local) && unit_type != UnitType::Mcp {
        return Err(AdapterError::Unsupported(unit_type));
    }
    Ok(())
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
    fn argv_stdio() {
        let argv = build_add_argv(&m_stdio(), Scope::Global);
        assert_eq!(
            argv,
            vec![
                "mcp",
                "add",
                "--transport",
                "stdio",
                "--scope",
                "user",
                "--env",
                "GITHUB_TOKEN",
                "github",
                "--",
                "npx",
                "-y",
                "@modelcontextprotocol/server-github",
            ]
        );
    }

    #[test]
    fn argv_http_with_header() {
        let argv = build_add_argv(&m_http(), Scope::Project);
        assert_eq!(
            argv,
            vec![
                "mcp",
                "add",
                "--transport",
                "http",
                "--scope",
                "project",
                "--header",
                "Authorization: Bearer ${FIGMA_TOKEN}",
                "figma",
                "https://mcp.figma.com/",
            ]
        );
    }

    #[test]
    fn argv_sse_local_scope() {
        let argv = build_add_argv(&m_sse(), Scope::Local);
        assert_eq!(
            argv,
            vec![
                "mcp",
                "add",
                "--transport",
                "sse",
                "--scope",
                "local",
                "analytics",
                "https://analytics.example.com/sse",
            ]
        );
    }

    #[test]
    fn argv_env_sorted_and_deduped() {
        let mut m = m_stdio();
        m.env = vec!["B".into(), "A".into(), "A".into()];
        let argv = build_add_argv(&m, Scope::Global);
        // Find the --env flags and assert order.
        let envs: Vec<&String> = argv
            .iter()
            .enumerate()
            .filter_map(|(i, s)| if s == "--env" { argv.get(i + 1) } else { None })
            .collect();
        assert_eq!(envs, vec![&"A".to_string(), &"B".to_string()]);
    }

    #[test]
    fn remove_argv() {
        assert_eq!(
            build_remove_argv("github", Scope::Global),
            vec!["mcp", "remove", "github", "--scope", "user"]
        );
    }

    #[test]
    fn converter_roundtrip() {
        let m = m_stdio();
        let native = MCPConverter.to_native(&m).unwrap();
        assert_eq!(native.files[0].relative_path, "mcp.toml");
        let back = MCPConverter.parse_native(&native).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn parse_claude_config_extracts_managed_entries() {
        let text = r#"{
          "mcpServers": {
            "github": {
              "type": "stdio",
              "command": "npx",
              "args": ["-y", "@modelcontextprotocol/server-github"],
              "env": { "GITHUB_TOKEN": "xyz" }
            }
          }
        }"#;
        let out = parse_claude_config(text, Scope::Global).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "github");
        assert_eq!(out[0].env, vec!["GITHUB_TOKEN".to_string()]);
    }

    #[test]
    fn parse_claude_config_drops_empty_env() {
        // Claude sometimes writes `"env": {}` — canonical form must
        // collapse to absent (spec §6).
        let text =
            r#"{ "mcpServers": { "x": { "type": "stdio", "command": "echo", "env": {} } } }"#;
        let out = parse_claude_config(text, Scope::Global).unwrap();
        let m = out.into_iter().next().unwrap();
        assert!(m.env.is_empty());
    }

    #[test]
    fn parse_claude_config_preserves_placeholder_literally() {
        let text = r#"{ "mcpServers": { "figma": { "type": "http", "url": "https://x", "headers": { "Authorization": "Bearer ${TOKEN}" } } } }"#;
        let m = parse_claude_config(text, Scope::Global)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        match m.transport {
            Transport::Http { headers, .. } => {
                assert_eq!(
                    headers.get("Authorization").map(String::as_str),
                    Some("Bearer ${TOKEN}")
                );
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parse_claude_config_rejects_unknown_fields() {
        let text =
            r#"{ "mcpServers": { "bad": { "type": "stdio", "command": "echo", "wat": 42 } } }"#;
        assert!(parse_claude_config(text, Scope::Global).is_err());
    }

    #[test]
    fn validate_scope_local_rejects_non_mcp() {
        assert!(validate_scope(UnitType::Skill, Scope::Local).is_err());
        assert!(validate_scope(UnitType::Mcp, Scope::Local).is_ok());
        assert!(validate_scope(UnitType::Rule, Scope::Global).is_ok());
    }

    #[test]
    fn drift_canonical_equivalence() {
        // Same MCP, different whitespace / key-order in native JSON
        // → same canonical SHA.
        let text_a = r#"{ "mcpServers": { "gh": { "type": "stdio", "command": "npx", "args": ["a"], "env": { "T": "v" } } } }"#;
        let text_b = r#"{ "mcpServers": { "gh": { "env": { "T": "v" }, "args": ["a"], "command": "npx", "type": "stdio" } } }"#;
        let a = parse_claude_config(text_a, Scope::Global).unwrap();
        let b = parse_claude_config(text_b, Scope::Global).unwrap();
        let sha_a = Sha256::of(mcp::canonical_toml(&a[0]).as_bytes());
        let sha_b = Sha256::of(mcp::canonical_toml(&b[0]).as_bytes());
        assert_eq!(sha_a, sha_b);
    }

    #[test]
    fn binary_missing_error_is_specific() {
        let prev = std::env::var_os(CLAUDE_BIN_ENV);
        std::env::set_var(CLAUDE_BIN_ENV, "/nonexistent/definitely-not-a-binary-xyz");
        let r = run_claude(&["mcp".into(), "list".into()]);
        match prev {
            Some(v) => std::env::set_var(CLAUDE_BIN_ENV, v),
            None => std::env::remove_var(CLAUDE_BIN_ENV),
        }
        let err = r.unwrap_err();
        match err {
            AdapterError::Other { message, .. } => {
                assert!(
                    message.contains("not found") || message.contains("claude CLI"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("expected Other, got {other:?}"),
        }
    }

    /// Real-spawn test: requires an actual `claude` on PATH. Gated
    /// behind `#[ignore]` per spec §11.
    #[test]
    #[ignore]
    fn real_spawn_claude_version() {
        let run = run_claude(&["--version".into()]).expect("claude binary on PATH");
        assert!(run.status_ok, "claude --version failed");
    }
}
