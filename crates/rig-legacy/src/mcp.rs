use std::path::PathBuf;

use crate::store::RigConfig;

/// Where an MCP config was read from
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpSource {
    pub label: String,
    pub path: PathBuf,
}

impl std::fmt::Display for McpSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label)
    }
}

#[derive(Debug, Clone)]
pub struct McpEntry {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub source: McpSource,
    pub is_disabled: bool,
}

impl McpEntry {
    /// Unique key for the disabled_mcps set: "source_path::name"
    pub fn disable_key(&self) -> String {
        format!("{}::{}", self.source.path.display(), self.name)
    }
}

pub fn scan(project_dir: Option<&PathBuf>) -> Vec<McpEntry> {
    scan_with_config(project_dir, None)
}

pub fn scan_with_config(project_dir: Option<&PathBuf>, config: Option<&RigConfig>) -> Vec<McpEntry> {
    let mut all = Vec::new();
    let home = dirs::home_dir().unwrap_or_default();

    // Claude global: ~/.mcp.json
    let cc_global = home.join(".mcp.json");
    if cc_global.exists() {
        let source = McpSource { label: "Global".into(), path: cc_global.clone() };
        if let Ok(servers) = parse_file(&cc_global, source, config) {
            all.extend(servers);
        }
    }

    // Claude dir: ~/.claude/.mcp.json
    let cc_claude_dir = home.join(".claude/.mcp.json");
    if cc_claude_dir.exists() {
        let source = McpSource { label: "Global (~/.claude)".into(), path: cc_claude_dir.clone() };
        if let Ok(servers) = parse_file(&cc_claude_dir, source, config) {
            all.extend(servers);
        }
    }

    if let Some(proj) = project_dir {
        let cc_proj = proj.join(".mcp.json");
        if cc_proj.exists() {
            let source = McpSource { label: "Project".into(), path: cc_proj.clone() };
            if let Ok(servers) = parse_file(&cc_proj, source, config) {
                all.extend(servers);
            }
        }
        let gsd = proj.join(".gsd/.mcp.json");
        if gsd.exists() {
            let source = McpSource { label: "GSD Project".into(), path: gsd.clone() };
            if let Ok(servers) = parse_file(&gsd, source, config) {
                all.extend(servers);
            }
        }
    }

    all.sort_by(|a, b| a.name.cmp(&b.name));
    all
}

fn parse_file(
    path: &PathBuf,
    source: McpSource,
    config: Option<&RigConfig>,
) -> Result<Vec<McpEntry>, anyhow::Error> {
    let content = std::fs::read_to_string(path)?;
    let val: serde_json::Value = serde_json::from_str(&content)?;

    let servers = match val.get("mcpServers").and_then(|v| v.as_object()) {
        Some(obj) => obj,
        None => return Ok(Vec::new()),
    };

    let mut entries = Vec::new();
    for (name, def) in servers {
        let command = def
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let args = def
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let mut entry = McpEntry {
            name: name.clone(),
            command,
            args,
            source: source.clone(),
            is_disabled: false,
        };

        // Check if soft-disabled in config
        if let Some(cfg) = config {
            entry.is_disabled = cfg.disabled_mcps.contains(&entry.disable_key());
        }

        entries.push(entry);
    }

    Ok(entries)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct McpSandbox {
        dir: std::path::PathBuf,
    }

    impl McpSandbox {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("rig-mcp-test-{}", name));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self { dir }
        }

        fn write_mcp(&self, filename: &str, json: &str) {
            fs::write(self.dir.join(filename), json).unwrap();
        }
    }

    impl Drop for McpSandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn sample_mcp_json() -> &'static str {
        r#"{
            "mcpServers": {
                "filesystem": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
                },
                "github": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-github"]
                }
            }
        }"#
    }

    #[test]
    fn test_parse_file_extracts_servers() {
        let sb = McpSandbox::new("basic");
        sb.write_mcp("mcp.json", sample_mcp_json());

        let source = McpSource {
            label: "Test".into(),
            path: sb.dir.join("mcp.json"),
        };
        let entries = parse_file(&sb.dir.join("mcp.json"), source, None).unwrap();

        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.name == "filesystem"));
        assert!(entries.iter().any(|e| e.name == "github"));
    }

    #[test]
    fn test_parse_file_extracts_command_and_args() {
        let sb = McpSandbox::new("cmdargs");
        sb.write_mcp("mcp.json", sample_mcp_json());

        let source = McpSource {
            label: "Test".into(),
            path: sb.dir.join("mcp.json"),
        };
        let entries = parse_file(&sb.dir.join("mcp.json"), source, None).unwrap();

        let fs_server = entries.iter().find(|e| e.name == "filesystem").unwrap();
        assert_eq!(fs_server.command, "npx");
        assert_eq!(fs_server.args, vec![
            "-y", "@modelcontextprotocol/server-filesystem", "/tmp"
        ]);
    }

    #[test]
    fn test_parse_file_empty_mcp_servers() {
        let sb = McpSandbox::new("empty");
        sb.write_mcp("mcp.json", r#"{"mcpServers": {}}"#);

        let source = McpSource {
            label: "Test".into(),
            path: sb.dir.join("mcp.json"),
        };
        let entries = parse_file(&sb.dir.join("mcp.json"), source, None).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_file_no_mcp_servers_key() {
        let sb = McpSandbox::new("nokey");
        sb.write_mcp("mcp.json", r#"{"otherKey": "value"}"#);

        let source = McpSource {
            label: "Test".into(),
            path: sb.dir.join("mcp.json"),
        };
        let entries = parse_file(&sb.dir.join("mcp.json"), source, None).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_file_invalid_json() {
        let sb = McpSandbox::new("badjson");
        sb.write_mcp("mcp.json", "this is not json {{{");

        let source = McpSource {
            label: "Test".into(),
            path: sb.dir.join("mcp.json"),
        };
        let result = parse_file(&sb.dir.join("mcp.json"), source, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_file_server_missing_command() {
        let sb = McpSandbox::new("nocmd");
        sb.write_mcp("mcp.json", r#"{"mcpServers": {"broken": {"args": ["x"]}}}"#);

        let source = McpSource {
            label: "Test".into(),
            path: sb.dir.join("mcp.json"),
        };
        let entries = parse_file(&sb.dir.join("mcp.json"), source, None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, ""); // defaults to empty
        assert_eq!(entries[0].args, vec!["x"]);
    }

    #[test]
    fn test_parse_file_server_missing_args() {
        let sb = McpSandbox::new("noargs");
        sb.write_mcp("mcp.json", r#"{"mcpServers": {"simple": {"command": "node"}}}"#);

        let source = McpSource {
            label: "Test".into(),
            path: sb.dir.join("mcp.json"),
        };
        let entries = parse_file(&sb.dir.join("mcp.json"), source, None).unwrap();
        assert_eq!(entries[0].args, Vec::<String>::new());
    }

    #[test]
    fn test_parse_file_disabled_via_config() {
        let sb = McpSandbox::new("disabled");
        sb.write_mcp("mcp.json", r#"{"mcpServers": {"my-server": {"command": "node"}}}"#);

        let source = McpSource {
            label: "Test".into(),
            path: sb.dir.join("mcp.json").clone(),
        };

        let mut config = RigConfig::default();
        config.disabled_mcps.insert(format!("{}::my-server", sb.dir.join("mcp.json").display()));

        let entries = parse_file(&sb.dir.join("mcp.json"), source, Some(&config)).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_disabled);
    }

    #[test]
    fn test_parse_file_not_disabled_by_default() {
        let sb = McpSandbox::new("notdisabled");
        sb.write_mcp("mcp.json", r#"{"mcpServers": {"srv": {"command": "node"}}}"#);

        let source = McpSource {
            label: "Test".into(),
            path: sb.dir.join("mcp.json"),
        };
        let entries = parse_file(&sb.dir.join("mcp.json"), source, None).unwrap();
        assert!(!entries[0].is_disabled);
    }

    #[test]
    fn test_disable_key_format() {
        let entry = McpEntry {
            name: "my-server".into(),
            command: "node".into(),
            args: vec![],
            source: McpSource {
                label: "Test".into(),
                path: PathBuf::from("/tmp/.mcp.json"),
            },
            is_disabled: false,
        };
        assert_eq!(entry.disable_key(), "/tmp/.mcp.json::my-server");
    }

    #[test]
    fn test_mcp_source_display() {
        let source = McpSource {
            label: "Global".into(),
            path: PathBuf::from("/home/user/.mcp.json"),
        };
        assert_eq!(format!("{}", source), "Global");
    }
}
