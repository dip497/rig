//! Subagent ⇄ `<scope>/agents/<name>.md`.

use rig_core::adapter::{AdapterError, AdapterResult};
use rig_core::converter::{Converter, NativeFile, NativeLayout};
use rig_core::unit::Subagent;

use crate::frontmatter;

pub struct SubagentConverter;

impl Converter<Subagent> for SubagentConverter {
    fn to_native(&self, canonical: &Subagent) -> AdapterResult<NativeLayout> {
        let tools = canonical.tools.join(", ");
        let mut pairs: Vec<(&str, &str)> = vec![
            ("name", &canonical.name),
            ("description", &canonical.description),
        ];
        if !canonical.tools.is_empty() {
            pairs.push(("tools", &tools));
        }
        if let Some(m) = &canonical.model {
            pairs.push(("model", m));
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

    fn parse_native(&self, native: &NativeLayout) -> AdapterResult<Subagent> {
        let f = native.files.first().ok_or_else(|| AdapterError::Other {
            message: "subagent: empty native layout".into(),
            source: None,
        })?;
        let text = std::str::from_utf8(&f.bytes).map_err(|e| AdapterError::Other {
            message: "subagent not UTF-8".into(),
            source: Some(Box::new(e)),
        })?;
        let (fm, body) = frontmatter::split(text).ok_or_else(|| AdapterError::Other {
            message: "subagent missing `---` frontmatter fence".into(),
            source: None,
        })?;
        let pairs = frontmatter::parse_flat(fm);
        let mut name = None;
        let mut description = None;
        let mut tools = Vec::new();
        let mut model = None;
        for (k, v) in pairs {
            match k.as_str() {
                "name" => name = Some(v),
                "description" => description = Some(v),
                "tools" => tools = split_list(&v),
                "model" => model = Some(v),
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
                message: "subagent missing `name` frontmatter".into(),
                source: None,
            })?;
        let description = description.unwrap_or_default();
        Ok(Subagent {
            name,
            description,
            tools,
            model,
            body: body.to_owned(),
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
    fn roundtrip() {
        let s = Subagent {
            name: "security-reviewer".into(),
            description: "reviews for security issues".into(),
            tools: vec!["Read".into(), "Grep".into()],
            model: Some("opus".into()),
            body: "you review code for vulns.\n".into(),
        };
        let native = SubagentConverter.to_native(&s).unwrap();
        let back = SubagentConverter.parse_native(&native).unwrap();
        assert_eq!(s, back);
    }
}
