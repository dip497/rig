//! Minimal YAML frontmatter writer/reader for `SKILL.md`-style files.
//!
//! We deliberately avoid a full YAML dependency. Claude Code skill
//! frontmatter is a flat map of string keys to string values (or
//! occasional string lists). Anything richer stays in the body.

/// Split a file into (frontmatter_block, body). Returns `None` if the
/// file does not open with a `---` fence.
#[must_use]
pub fn split(s: &str) -> Option<(&str, &str)> {
    let s = s
        .strip_prefix("---\n")
        .or_else(|| s.strip_prefix("---\r\n"))?;
    let end = s.find("\n---\n").or_else(|| s.find("\n---\r\n"))?;
    let fm = &s[..end];
    let rest_start = end + s[end..].find("---").unwrap() + 3;
    let body = s[rest_start..]
        .strip_prefix('\n')
        .unwrap_or(&s[rest_start..]);
    let body = body.strip_prefix('\r').unwrap_or(body);
    let body = body.strip_prefix('\n').unwrap_or(body);
    Some((fm, body))
}

/// Parse flat `key: value` frontmatter lines. Quoted values are
/// unwrapped; unknown / malformed lines are skipped silently so we
/// round-trip defensively.
#[must_use]
pub fn parse_flat(block: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in block.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let key = k.trim().to_owned();
        let value = v.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
            .unwrap_or(value)
            .to_owned();
        out.push((key, value));
    }
    out
}

/// Render a flat `key: value` frontmatter block. Values containing
/// a colon, `#`, or leading/trailing whitespace get double-quoted.
#[must_use]
pub fn render_flat(pairs: &[(&str, &str)]) -> String {
    let mut out = String::from("---\n");
    for (k, v) in pairs {
        out.push_str(k);
        out.push_str(": ");
        if needs_quote(v) {
            out.push('"');
            for c in v.chars() {
                if c == '"' || c == '\\' {
                    out.push('\\');
                }
                out.push(c);
            }
            out.push('"');
        } else {
            out.push_str(v);
        }
        out.push('\n');
    }
    out.push_str("---\n");
    out
}

fn needs_quote(v: &str) -> bool {
    v.is_empty()
        || v.trim() != v
        || v.contains(':')
        || v.contains('#')
        || v.contains('"')
        || v.contains('\n')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let src = "---\nname: foo\ndescription: does a thing\n---\nbody here\n";
        let (fm, body) = split(src).unwrap();
        let kv = parse_flat(fm);
        assert_eq!(kv[0], ("name".into(), "foo".into()));
        assert_eq!(kv[1], ("description".into(), "does a thing".into()));
        assert_eq!(body, "body here\n");

        let rendered = render_flat(&[("name", "foo"), ("description", "does a thing")]);
        assert_eq!(rendered, "---\nname: foo\ndescription: does a thing\n---\n");
    }

    #[test]
    fn quotes_values_with_colons() {
        let r = render_flat(&[("description", "a: tricky")]);
        assert!(r.contains("description: \"a: tricky\""));
    }

    #[test]
    fn no_frontmatter_returns_none() {
        assert!(split("no fence here\n").is_none());
    }

    #[test]
    fn handles_quoted_input() {
        let kv = parse_flat("description: \"hello: world\"");
        assert_eq!(kv[0].1, "hello: world");
    }
}
