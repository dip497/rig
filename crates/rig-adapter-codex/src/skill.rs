//! Skill ⇄ `SKILL.md` translation.
//!
//! Codex reads skills from `<scope>/skills/<name>/SKILL.md`. The SKILL.md
//! format is the open standard shared with Claude Code — same frontmatter
//! schema, same layout.

use rig_core::adapter::{AdapterError, AdapterResult};
use rig_core::converter::{Converter, NativeFile, NativeLayout};
use rig_core::unit::Skill;

use crate::frontmatter;

/// Canonical path of the primary file inside a skill directory.
pub const SKILL_FILE: &str = "SKILL.md";

pub struct SkillConverter;

impl Converter<Skill> for SkillConverter {
    fn to_native(&self, canonical: &Skill) -> AdapterResult<NativeLayout> {
        let fm = frontmatter::render_flat(&[
            ("name", &canonical.name),
            ("description", &canonical.description),
        ]);
        let mut contents = fm;
        contents.push('\n');
        contents.push_str(&canonical.body);

        let mut files = Vec::with_capacity(1 + canonical.resources.len());
        files.push(NativeFile {
            relative_path: SKILL_FILE.to_owned(),
            bytes: contents.into_bytes(),
        });
        for r in &canonical.resources {
            files.push(NativeFile {
                relative_path: r.path.clone(),
                bytes: r.bytes.clone(),
            });
        }
        Ok(NativeLayout { files })
    }

    fn parse_native(&self, native: &NativeLayout) -> AdapterResult<Skill> {
        let skill_file = native
            .files
            .iter()
            .find(|f| f.relative_path == SKILL_FILE)
            .ok_or_else(|| AdapterError::Other {
                message: format!("missing {SKILL_FILE}"),
                source: None,
            })?;

        let text = std::str::from_utf8(&skill_file.bytes).map_err(|e| AdapterError::Other {
            message: "SKILL.md is not UTF-8".into(),
            source: Some(Box::new(e)),
        })?;

        let (fm_block, body) = frontmatter::split(text).ok_or_else(|| AdapterError::Other {
            message: "SKILL.md missing `---` frontmatter fence".into(),
            source: None,
        })?;

        let pairs = frontmatter::parse_flat(fm_block);
        let mut name = None;
        let mut description = None;
        for (k, v) in pairs {
            match k.as_str() {
                "name" => name = Some(v),
                "description" => description = Some(v),
                _ => {} // extra frontmatter ignored for now
            }
        }
        let name = name.ok_or_else(|| AdapterError::Other {
            message: "SKILL.md frontmatter missing `name`".into(),
            source: None,
        })?;
        let description = description.unwrap_or_default();

        let resources = native
            .files
            .iter()
            .filter(|f| f.relative_path != SKILL_FILE)
            .map(|f| rig_core::unit::skill::Resource {
                path: f.relative_path.clone(),
                bytes: f.bytes.clone(),
            })
            .collect();

        Ok(Skill {
            name,
            description,
            extra_frontmatter: Default::default(),
            body: body.to_owned(),
            resources,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rig_core::unit::skill::Resource;

    fn sample() -> Skill {
        Skill {
            name: "react-review".into(),
            description: "reviews React components".into(),
            extra_frontmatter: Default::default(),
            body: "# React review\n\nSteps:\n- ...\n".into(),
            resources: vec![Resource {
                path: "references/patterns.md".into(),
                bytes: b"# patterns\n".to_vec(),
            }],
        }
    }

    #[test]
    fn roundtrip() {
        let s = sample();
        let native = SkillConverter.to_native(&s).unwrap();
        assert_eq!(native.files[0].relative_path, "SKILL.md");
        assert_eq!(native.files[1].relative_path, "references/patterns.md");
        let back = SkillConverter.parse_native(&native).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn rejects_missing_frontmatter() {
        let native = NativeLayout {
            files: vec![NativeFile {
                relative_path: "SKILL.md".into(),
                bytes: b"no frontmatter".to_vec(),
            }],
        };
        assert!(SkillConverter.parse_native(&native).is_err());
    }

    #[test]
    fn rejects_missing_name() {
        let native = NativeLayout {
            files: vec![NativeFile {
                relative_path: "SKILL.md".into(),
                bytes: b"---\ndescription: d\n---\nbody\n".to_vec(),
            }],
        };
        assert!(SkillConverter.parse_native(&native).is_err());
    }
}
