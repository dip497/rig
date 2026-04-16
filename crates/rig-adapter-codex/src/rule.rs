//! Rule ⇄ `<scope>/rules/<name>.md`.
//!
//! Codex's canonical rules file is `AGENTS.md`, but Rig cannot safely
//! append multiple rules into one file and still round-trip them. For
//! M1 we mirror the Claude adapter: each rule gets its own
//! `<scope>/rules/<name>.md` side-file and the user references it from
//! AGENTS.md as needed. This keeps Rig-managed rules cleanly separable
//! from hand-written AGENTS.md content — same rationale as the Claude
//! adapter which side-files rules and lets the user @import into
//! CLAUDE.md.

use rig_core::adapter::{AdapterError, AdapterResult};
use rig_core::converter::{Converter, NativeFile, NativeLayout};
use rig_core::unit::Rule;

use crate::frontmatter;

pub struct RuleConverter;

impl Converter<Rule> for RuleConverter {
    fn to_native(&self, canonical: &Rule) -> AdapterResult<NativeLayout> {
        let mut pairs: Vec<(&str, &str)> = vec![("name", &canonical.name)];
        if let Some(d) = &canonical.description {
            pairs.push(("description", d));
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

    fn parse_native(&self, native: &NativeLayout) -> AdapterResult<Rule> {
        let f = native.files.first().ok_or_else(|| AdapterError::Other {
            message: "rule: empty native layout".into(),
            source: None,
        })?;
        let text = std::str::from_utf8(&f.bytes).map_err(|e| AdapterError::Other {
            message: "rule not UTF-8".into(),
            source: Some(Box::new(e)),
        })?;
        let (fm, body) = frontmatter::split(text).ok_or_else(|| AdapterError::Other {
            message: "rule missing `---` frontmatter fence".into(),
            source: None,
        })?;
        let pairs = frontmatter::parse_flat(fm);
        let mut name = None;
        let mut description = None;
        for (k, v) in pairs {
            match k.as_str() {
                "name" => name = Some(v),
                "description" => description = Some(v),
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
                message: "rule missing `name` frontmatter".into(),
                source: None,
            })?;
        Ok(Rule {
            name,
            description,
            body: body.to_owned(),
            placement: Default::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let r = Rule {
            name: "typescript-style".into(),
            description: Some("TS conventions".into()),
            body: "use `const` over `let`.\n".into(),
            placement: Default::default(),
        };
        let native = RuleConverter.to_native(&r).unwrap();
        assert_eq!(native.files[0].relative_path, "typescript-style.md");
        let back = RuleConverter.parse_native(&native).unwrap();
        assert_eq!(r, back);
    }
}
