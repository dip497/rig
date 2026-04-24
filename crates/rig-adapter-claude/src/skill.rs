//! Skill ⇄ `SKILL.md` translation.

use rig_core::adapter::{AdapterError, AdapterResult};
use rig_core::converter::{Converter, NativeFile, NativeLayout};
use rig_core::unit::Skill;

use crate::frontmatter;

/// Canonical path of the primary file inside a skill directory.
pub const SKILL_FILE: &str = "SKILL.md";

pub struct SkillConverter;

impl Converter<Skill> for SkillConverter {
    fn to_native(&self, canonical: &Skill) -> AdapterResult<NativeLayout> {
        // Name + description are always emitted first, followed by any
        // extra frontmatter (string values only — richer values live
        // in the body per the M1 skill schema). Preserving these
        // keys on round-trip is what lets `rig disable` survive
        // `rig enable` without losing agent-native extensions.
        let mut pairs: Vec<(&str, String)> = Vec::new();
        pairs.push(("name", canonical.name.clone()));
        pairs.push(("description", canonical.description.clone()));
        for (k, v) in &canonical.extra_frontmatter {
            let s = match v {
                toml::Value::String(s) => s.clone(),
                toml::Value::Boolean(b) => b.to_string(),
                toml::Value::Integer(i) => i.to_string(),
                toml::Value::Float(f) => f.to_string(),
                other => other.to_string(),
            };
            pairs.push((k.as_str(), s));
        }
        let borrowed: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let fm = frontmatter::render_flat(&borrowed);
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
        let mut extra: std::collections::BTreeMap<String, toml::Value> =
            std::collections::BTreeMap::new();
        for (k, v) in pairs {
            match k.as_str() {
                "name" => name = Some(v),
                "description" => description = Some(v),
                // Drop Rig's own disable sentinel when parsing back;
                // the adapter tracks disable via `is_enabled()`, not
                // the canonical `Skill` struct.
                "disable-model-invocation" => {}
                k if k.starts_with("rig-disabled-") => {}
                _ => {
                    extra.insert(k, toml::Value::String(v));
                }
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
            extra_frontmatter: extra,
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
