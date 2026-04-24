//! Codex MCP adapter logic (see `docs/MCP-SUPPORT.md` §5).
//!
//! Codex supports `stdio` and `http` transports, global scope only, via
//! `codex mcp add|remove|list`. Rig never hand-edits
//! `~/.codex/config.toml`. Drift detection re-canonicalises the stored
//! entry so whitespace/key-order noise doesn't trip `LocalDrift`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use rig_core::adapter::{AdapterError, AdapterResult, UnitRef};
use rig_core::converter::{Converter, NativeFile, NativeLayout};
use rig_core::scope::Scope;
use rig_core::source::Sha256;
use rig_core::unit::mcp::{self, Mcp, Transport};
use rig_core::unit::UnitType;

pub(crate) const CODEX_BIN_ENV: &str = "RIG_CODEX_BIN";

pub struct MCPConverter;

impl Converter<Mcp> for MCPConverter {
    fn to_native(&self, canonical: &Mcp) -> AdapterResult<NativeLayout> {
        // Share the source-form serialiser with the Claude adapter by
        // copying the logic here (cross-adapter imports forbidden).
        Ok(NativeLayout {
            files: vec![NativeFile {
                relative_path: "mcp.toml".into(),
                bytes: surface_toml(canonical).into_bytes(),
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

fn surface_toml(m: &Mcp) -> String {
    let mut out = String::new();
    out.push_str("schema = \"rig/v1\"\n");
    out.push_str("kind = \"mcp\"\n\n");
    out.push_str(&format!("name = {}\n", q(&m.name)));
    if let Some(d) = &m.description {
        out.push_str(&format!("description = {}\n", q(d)));
    }
    // Emit env before `[transport]` so it stays at top-level.
    if !m.env.is_empty() {
        out.push_str("env = [");
        for (i, e) in m.env.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&q(e));
        }
        out.push_str("]\n");
    }
    out.push_str("\n[transport]\n");
    match &m.transport {
        Transport::Stdio { command, args } => {
            out.push_str("kind = \"stdio\"\n");
            out.push_str(&format!("command = {}\n", q(command)));
            if !args.is_empty() {
                out.push_str("args = [");
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&q(a));
                }
                out.push_str("]\n");
            }
        }
        Transport::Http { url, headers } => {
            out.push_str("kind = \"http\"\n");
            out.push_str(&format!("url = {}\n", q(url)));
            if !headers.is_empty() {
                out.push_str("headers = { ");
                for (i, (k, v)) in headers.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&format!("{k} = {}", q(v)));
                }
                out.push_str(" }\n");
            }
        }
        Transport::Sse { url, headers } => {
            out.push_str("kind = \"sse\"\n");
            out.push_str(&format!("url = {}\n", q(url)));
            if !headers.is_empty() {
                out.push_str("headers = { ");
                for (i, (k, v)) in headers.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&format!("{k} = {}", q(v)));
                }
                out.push_str(" }\n");
            }
        }
    }
    out
}

fn q(s: &str) -> String {
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

// ---------------- probe ----------------

/// One-shot probe at adapter-construction time: does `codex mcp` exist?
/// Result is cached on the adapter so `capabilities()` stays O(1).
pub(crate) fn probe_supported() -> bool {
    #[cfg(test)]
    {
        crate::PROBE_CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    let bin = std::env::var(CODEX_BIN_ENV).unwrap_or_else(|_| "codex".to_string());
    let out = Command::new(&bin)
        .args(["mcp", "--help"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    matches!(out, Ok(s) if s.success())
}

// ---------------- scope + argv ----------------

pub(crate) fn ensure_global(scope: Scope) -> AdapterResult<()> {
    if matches!(scope, Scope::Global) {
        Ok(())
    } else {
        Err(AdapterError::Unsupported(UnitType::Mcp))
    }
}

/// Pure argv builder for `codex mcp add`. Codex supports only stdio
/// and http; SSE callers must be rejected before reaching this fn.
#[must_use]
pub fn build_add_argv(mcp: &Mcp) -> Vec<String> {
    let mut argv: Vec<String> = vec!["mcp".into(), "add".into()];
    argv.push("--transport".into());
    argv.push(mcp.transport.tag().to_owned());

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
        Transport::Http { url, headers } => {
            for (k, v) in headers {
                argv.push("--header".into());
                argv.push(format!("{k}: {v}"));
            }
            argv.push(mcp.name.clone());
            argv.push(url.clone());
        }
        // Caller rejects SSE before we reach here; keep it a no-op
        // so enum coverage remains exhaustive.
        Transport::Sse { url, headers } => {
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

#[must_use]
pub fn build_remove_argv(name: &str) -> Vec<String> {
    vec!["mcp".into(), "remove".into(), name.to_owned()]
}

// ---------------- process invocation ----------------

#[derive(Debug)]
pub(crate) struct CliRun {
    pub status_ok: bool,
    #[allow(dead_code)]
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub(crate) fn run_codex(argv: &[String]) -> AdapterResult<CliRun> {
    let bin = std::env::var(CODEX_BIN_ENV).unwrap_or_else(|_| "codex".to_string());
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
            message: "codex CLI not found on PATH".into(),
            source: Some(Box::new(e)),
        }),
        Err(e) => Err(AdapterError::Other {
            message: format!("invoking `{bin}`: {e}"),
            source: Some(Box::new(e)),
        }),
    }
}

// ---------------- install / uninstall ----------------

pub(crate) fn install(mcp: &Mcp, scope: Scope) -> AdapterResult<(Sha256, PathBuf)> {
    ensure_global(scope)?;
    if matches!(mcp.transport, Transport::Sse { .. }) {
        return Err(AdapterError::Unsupported(UnitType::Mcp));
    }
    let _ = run_codex(&build_remove_argv(&mcp.name));
    let run = run_codex(&build_add_argv(mcp))?;
    if !run.status_ok {
        return Err(AdapterError::Other {
            message: format!(
                "codex mcp add failed: {}",
                String::from_utf8_lossy(&run.stderr).trim()
            ),
            source: None,
        });
    }
    let install_sha = Sha256::of(mcp::canonical_toml(mcp).as_bytes());
    Ok((install_sha, config_path()?))
}

pub(crate) fn uninstall(name: &str, scope: Scope) -> AdapterResult<()> {
    ensure_global(scope)?;
    let run = run_codex(&build_remove_argv(name))?;
    if run.status_ok {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&run.stderr).to_ascii_lowercase();
    if stderr.contains("not found") || stderr.contains("no such") {
        return Ok(());
    }
    Err(AdapterError::Other {
        message: format!(
            "codex mcp remove failed: {}",
            String::from_utf8_lossy(&run.stderr).trim()
        ),
        source: None,
    })
}

// ---------------- list + read + drift ----------------

fn config_path() -> AdapterResult<PathBuf> {
    let home = rig_fs::home_dir().map_err(|e| AdapterError::Other {
        message: e.to_string(),
        source: Some(Box::new(e)),
    })?;
    Ok(home.join(".codex").join("config.toml"))
}

pub(crate) fn list_native() -> AdapterResult<Vec<Mcp>> {
    let p = config_path()?;
    if !p.exists() {
        return Ok(Vec::new());
    }
    let bytes = std::fs::read(&p).map_err(|e| AdapterError::Other {
        message: format!("reading {}: {e}", p.display()),
        source: Some(Box::new(e)),
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|e| AdapterError::Other {
        message: format!("{} is not UTF-8: {e}", p.display()),
        source: None,
    })?;
    parse_codex_config(text)
}

/// Parse `~/.codex/config.toml` into canonical entries. Codex stores
/// MCPs under `[mcp_servers.<name>]` tables.
pub(crate) fn parse_codex_config(text: &str) -> AdapterResult<Vec<Mcp>> {
    let v: toml::Value = toml::from_str(text).map_err(|e| AdapterError::Other {
        message: format!("parsing codex config: {e}"),
        source: Some(Box::new(e)),
    })?;
    let mut out = Vec::new();
    let Some(servers) = v.get("mcp_servers").and_then(|s| s.as_table()) else {
        return Ok(out);
    };
    for (name, entry) in servers {
        let tbl = entry.as_table().ok_or_else(|| AdapterError::Other {
            message: format!("codex mcp `{name}` is not a table"),
            source: None,
        })?;
        let m = native_to_canonical(name, tbl)?;
        out.push(m);
    }
    Ok(out)
}

fn native_to_canonical(name: &str, tbl: &toml::value::Table) -> AdapterResult<Mcp> {
    const KNOWN: &[&str] = &[
        "type",
        "transport",
        "command",
        "args",
        "url",
        "headers",
        "env",
    ];
    let unknown: Vec<&str> = tbl
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

    let tag = tbl
        .get("transport")
        .or_else(|| tbl.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("stdio");

    let env: Vec<String> = tbl
        .get("env")
        .and_then(|v| v.as_table())
        .map(|m| {
            let mut keys: Vec<String> = m.keys().cloned().collect();
            keys.sort();
            keys
        })
        .unwrap_or_default();

    let headers: BTreeMap<String, String> = tbl
        .get("headers")
        .and_then(|v| v.as_table())
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                .collect()
        })
        .unwrap_or_default();

    let transport = match tag {
        "stdio" => {
            let command = tbl
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AdapterError::Other {
                    message: format!("codex mcp `{name}`: stdio entry missing `command`"),
                    source: None,
                })?
                .to_owned();
            let args: Vec<String> = tbl
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
            let url = tbl
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AdapterError::Other {
                    message: format!("codex mcp `{name}`: http entry missing `url`"),
                    source: None,
                })?
                .to_owned();
            Transport::Http { url, headers }
        }
        other => {
            return Err(AdapterError::Other {
                message: format!("codex mcp `{name}`: unsupported transport `{other}`"),
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

pub(crate) fn read_local(unit_ref: &UnitRef, scope: Scope) -> AdapterResult<Mcp> {
    ensure_global(scope)?;
    let all = list_native()?;
    all.into_iter()
        .find(|m| m.name == unit_ref.name)
        .ok_or_else(|| AdapterError::NotFound(unit_ref.name.clone(), scope))
}

pub(crate) fn current_sha(name: &str, scope: Scope) -> AdapterResult<Option<Sha256>> {
    ensure_global(scope)?;
    let all = list_native()?;
    Ok(all
        .into_iter()
        .find(|m| m.name == name)
        .map(|m| Sha256::of(mcp::canonical_toml(&m).as_bytes())))
}

pub(crate) fn config_path_public(scope: Scope) -> AdapterResult<PathBuf> {
    ensure_global(scope)?;
    config_path()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stdio() -> Mcp {
        Mcp {
            name: "gh".into(),
            description: None,
            transport: Transport::Stdio {
                command: "npx".into(),
                args: vec!["-y".into(), "server".into()],
            },
            env: vec!["TOKEN".into()],
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn argv_stdio() {
        let argv = build_add_argv(&stdio());
        assert_eq!(
            argv,
            vec![
                "mcp",
                "add",
                "--transport",
                "stdio",
                "--env",
                "TOKEN",
                "gh",
                "--",
                "npx",
                "-y",
                "server",
            ]
        );
    }

    #[test]
    fn argv_http() {
        let mut h = BTreeMap::new();
        h.insert("X-K".into(), "v".into());
        let m = Mcp {
            name: "f".into(),
            description: None,
            transport: Transport::Http {
                url: "https://x".into(),
                headers: h,
            },
            env: Vec::new(),
            metadata: BTreeMap::new(),
        };
        let argv = build_add_argv(&m);
        assert_eq!(
            argv,
            vec![
                "mcp",
                "add",
                "--transport",
                "http",
                "--header",
                "X-K: v",
                "f",
                "https://x"
            ]
        );
    }

    #[test]
    fn remove_argv() {
        assert_eq!(build_remove_argv("gh"), vec!["mcp", "remove", "gh"]);
    }

    #[test]
    fn converter_roundtrip() {
        let native = MCPConverter.to_native(&stdio()).unwrap();
        let back = MCPConverter.parse_native(&native).unwrap();
        assert_eq!(back, stdio());
    }

    #[test]
    fn parse_codex_config_basic() {
        let text = r#"
[mcp_servers.gh]
type = "stdio"
command = "npx"
args = ["-y", "server"]
[mcp_servers.gh.env]
TOKEN = "v"
"#;
        let out = parse_codex_config(text).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "gh");
        assert_eq!(out[0].env, vec!["TOKEN".to_string()]);
    }

    #[test]
    fn parse_codex_rejects_unknown() {
        let text = r#"
[mcp_servers.bad]
type = "stdio"
command = "x"
wat = 1
"#;
        assert!(parse_codex_config(text).is_err());
    }

    #[test]
    fn ensure_global_rejects_other_scopes() {
        assert!(ensure_global(Scope::Project).is_err());
        assert!(ensure_global(Scope::Local).is_err());
        assert!(ensure_global(Scope::Global).is_ok());
    }

    #[test]
    fn binary_missing_error_specific() {
        let prev = std::env::var_os(CODEX_BIN_ENV);
        std::env::set_var(CODEX_BIN_ENV, "/nonexistent/codex-xyz");
        let r = run_codex(&["--version".into()]);
        match prev {
            Some(v) => std::env::set_var(CODEX_BIN_ENV, v),
            None => std::env::remove_var(CODEX_BIN_ENV),
        }
        let err = r.unwrap_err();
        match err {
            AdapterError::Other { message, .. } => {
                assert!(message.contains("not found") || message.contains("codex CLI"));
            }
            other => panic!("expected Other, got {other:?}"),
        }
    }
}
