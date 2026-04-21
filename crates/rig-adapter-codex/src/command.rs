//! Command ⇄ `<scope>/commands/<name>.md`.
//!
//! Codex consumes the same SKILL-family frontmatter format as Claude
//! Code for slash-style commands until Codex docs diverge. See
//! `../../rig-adapter-claude/src/command.rs` for the Claude variant.

use rig_core::adapter::{AdapterError, AdapterResult};
use rig_core::converter::{Converter, NativeFile, NativeLayout};
use rig_core::unit::Command;

use crate::frontmatter;

pub struct CommandConverter;

impl Converter<Command> for CommandConverter {
    fn to_native(&self, canonical: &Command) -> AdapterResult<NativeLayout> {
        let tools = canonical.tools.join(", ");
        let mut pairs: Vec<(&str, &str)> = vec![("name", &canonical.name)];
        if let Some(d) = &canonical.description {
            pairs.push(("description", d));
        }
        if !canonical.tools.is_empty() {
            pairs.push(("tools", &tools));
        }
        let mut contents = frontmatter::render_flat(&pairs);
        contents.push('\n');
        contents.push_str(&canonical.body);
        Ok(NativeLayout {
            files: vec![NativeFile {
                relative_path: format!("{}.md", canonical.name),
                bytes: contents.into_bytes(),
            }],
        })
    }

    fn parse_native(&self, native: &NativeLayout) -> AdapterResult<Command> {
        let f = native.files.first().ok_or_else(|| AdapterError::Other {
            message: "command: empty native layout".into(),
            source: None,
        })?;
        let text = std::str::from_utf8(&f.bytes).map_err(|e| AdapterError::Other {
            message: "command not UTF-8".into(),
            source: Some(Box::new(e)),
        })?;
        let (fm, body) = frontmatter::split(text).ok_or_else(|| AdapterError::Other {
            message: "command missing `---` frontmatter fence".into(),
            source: None,
        })?;
        let pairs = frontmatter::parse_flat(fm);
        let mut name = None;
        let mut description = None;
        let mut tools = Vec::new();
        for (k, v) in pairs {
            match k.as_str() {
                "name" => name = Some(v),
                "description" => description = Some(v),
                "tools" => tools = split_list(&v),
                _ => {}
            }
        }
        let name = name
            .or_else(|| {
                f.relative_path
                    .strip_suffix(".md")
                    .map(std::string::ToString::to_string)
            })
            .ok_or_else(|| AdapterError::Other {
                message: "command missing `name` frontmatter".into(),
                source: None,
            })?;
        Ok(Command {
            name,
            description,
            body: body.to_owned(),
            tools,
        })
    }
}

fn split_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_with_tools() {
        let c = Command {
            name: "review".into(),
            description: Some("review changes".into()),
            body: "review the diff.\n".into(),
            tools: vec!["Read".into(), "Grep".into()],
        };
        let native = CommandConverter.to_native(&c).unwrap();
        let back = CommandConverter.parse_native(&native).unwrap();
        assert_eq!(c, back);
    }
}
