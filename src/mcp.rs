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
