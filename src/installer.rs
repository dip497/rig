/// Skill installer — install, outdated check, and update.
///
/// Supports:
///   github:owner/repo
///   github:owner/repo#ref
///   github:owner/repo#ref:subpath
///   owner/repo               (shorthand)
///   https://github.com/owner/repo[/tree/branch/path]
///   /absolute/path or ./relative/path  (local)
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::lock::{self, LockEntry};
use crate::scanner;
use crate::store::{self, Agent, RigConfig};

// ── Source parsing ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ParsedSource {
    pub owner: String,
    pub repo: String,
    /// Branch / tag / commit
    pub git_ref: Option<String>,
    /// Subdirectory within the repo (for multi-skill repos)
    pub subpath: Option<String>,
    pub clone_url: String,
    pub ssh_url: String,
    pub is_local: bool,
    pub local_path: Option<PathBuf>,
}

impl ParsedSource {
    pub fn canonical_id(&self) -> String {
        if self.is_local {
            format!("local:{}", self.local_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default())
        } else {
            let mut s = format!("github:{}/{}", self.owner, self.repo);
            if let Some(r) = &self.git_ref {
                s.push('#');
                s.push_str(r);
            }
            if let Some(p) = &self.subpath {
                s.push(':');
                s.push_str(p);
            }
            s
        }
    }
}

/// Parse any supported source format into a `ParsedSource`.
pub fn parse_source(input: &str) -> Result<ParsedSource> {
    let input = input.trim();

    // Local paths
    if input.starts_with('/')
        || input.starts_with("./")
        || input.starts_with("../")
        || input.starts_with("~/")
    {
        let expanded = shellexpand::tilde(input).to_string();
        let path = PathBuf::from(&expanded);
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        return Ok(ParsedSource {
            owner: "local".into(),
            repo: name,
            git_ref: None,
            subpath: None,
            clone_url: String::new(),
            ssh_url: String::new(),
            is_local: true,
            local_path: Some(path),
        });
    }

    // Normalise HTTPS GitHub URL → github:owner/repo[#ref[:subpath]]
    let input = if let Some(rest) = input
        .strip_prefix("https://github.com/")
        .or_else(|| input.strip_prefix("http://github.com/"))
    {
        // rest could be: owner/repo  or  owner/repo/tree/branch/sub/path
        let rest = rest.trim_end_matches('/');
        if let Some(tree_pos) = rest.find("/tree/") {
            let owner_repo = &rest[..tree_pos];
            let after_tree = &rest[tree_pos + 6..]; // skip "/tree/"
            // after_tree = "branch"  or  "branch/sub/path"
            let slash = after_tree.find('/');
            if let Some(pos) = slash {
                let branch = &after_tree[..pos];
                let sub = &after_tree[pos + 1..];
                format!("github:{}#{branch}:{sub}", owner_repo)
            } else {
                format!("github:{}#{after_tree}", owner_repo)
            }
        } else {
            format!("github:{rest}")
        }
    } else {
        input.to_string()
    };

    // Strip "github:" prefix (or treat bare "owner/repo" as github shorthand)
    let rest = if let Some(r) = input.strip_prefix("github:") {
        r
    } else if input.contains('/') && !input.contains(':') {
        &input
    } else {
        bail!(
            "Unrecognised source format: {input:?}\n\
             Supported:\n\
             \x20 github:owner/repo[#ref[:subpath]]\n\
             \x20 owner/repo\n\
             \x20 https://github.com/owner/repo[/tree/branch/path]\n\
             \x20 /absolute/path  or  ./relative/path"
        );
    };

    // rest = "owner/repo[#ref[:subpath]]"  or  "owner/repo[:subpath]" (no ref)
    let (owner_repo, git_ref, subpath) = {
        if let Some(hash_pos) = rest.find('#') {
            let owner_repo = &rest[..hash_pos];
            let ref_and_path = &rest[hash_pos + 1..];
            if let Some(colon_pos) = ref_and_path.find(':') {
                let git_ref = &ref_and_path[..colon_pos];
                let sub = &ref_and_path[colon_pos + 1..];
                (owner_repo, Some(git_ref.to_string()), if sub.is_empty() { None } else { Some(sub.to_string()) })
            } else {
                (owner_repo, Some(ref_and_path.to_string()), None)
            }
        } else if let Some(colon_pos) = rest.find(':') {
            let owner_repo = &rest[..colon_pos];
            let sub = &rest[colon_pos + 1..];
            (owner_repo, None, if sub.is_empty() { None } else { Some(sub.to_string()) })
        } else {
            (rest, None, None)
        }
    };

    let slash = owner_repo.find('/').context("Expected owner/repo format")?;
    let owner = &owner_repo[..slash];
    let repo_raw = &owner_repo[slash + 1..];
    let repo = repo_raw.trim_end_matches(".git");

    validate_name(owner, "owner")?;
    validate_name(repo, "repo")?;

    Ok(ParsedSource {
        owner: owner.to_string(),
        repo: repo.to_string(),
        git_ref,
        subpath,
        clone_url: format!("https://github.com/{owner}/{repo}.git"),
        ssh_url: format!("git@github.com:{owner}/{repo}.git"),
        is_local: false,
        local_path: None,
    })
}

fn validate_name(s: &str, label: &str) -> Result<()> {
    if s.is_empty() {
        bail!("Invalid source: {label} cannot be empty");
    }
    if s.contains("..") || s.contains('/') || s.contains('\\') || s.contains('\0') {
        bail!("Invalid source: {label} contains unsafe characters");
    }
    if s.len() > 128 {
        bail!("Invalid source: {label} is too long");
    }
    Ok(())
}

// ── Skill discovery ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DiscoveredSkill {
    /// Path relative to the clone root
    pub rel_path: String,
    /// Skill name from SKILL.md frontmatter (falls back to dir name)
    pub name: String,
    /// Short description from frontmatter
    pub description: String,
}

/// Walk a cloned repo directory and find all `SKILL.md` files.
/// Returns skills sorted by name.
pub fn discover_skills(root: &Path) -> Vec<DiscoveredSkill> {
    let mut skills = Vec::new();
    discover_recursive(root, root, &mut skills, 0);
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

fn discover_recursive(
    base: &Path,
    dir: &Path,
    out: &mut Vec<DiscoveredSkill>,
    depth: usize,
) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" || name == ".git" {
            continue;
        }
        if !path.is_dir() {
            continue;
        }
        let skill_md = path.join("SKILL.md");
        if skill_md.exists() {
            let rel = path.strip_prefix(base).unwrap_or(&path).to_string_lossy().to_string();
            let (skill_name, description) = parse_skill_md(&skill_md, &name);
            out.push(DiscoveredSkill { rel_path: rel, name: skill_name, description });
            // Don't recurse into skill dirs
        } else {
            discover_recursive(base, &path, out, depth + 1);
        }
    }
}

fn parse_skill_md(path: &Path, fallback_name: &str) -> (String, String) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (fallback_name.to_string(), String::new()),
    };
    let name = extract_frontmatter_field(&content, "name")
        .unwrap_or_else(|| fallback_name.to_string());
    let desc = extract_frontmatter_field(&content, "description").unwrap_or_default();
    (name, desc)
}

fn extract_frontmatter_field(content: &str, field: &str) -> Option<String> {
    let after_open = content.strip_prefix("---")?;
    let end = after_open.find("\n---")?;
    let fm = &after_open[..end];
    for line in fm.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(field) {
            if let Some(rest) = rest.strip_prefix(':') {
                let val = rest.trim().trim_matches('"').trim_matches('\'').to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }
    None
}

// ── Temp directory (auto-cleanup on drop) ────────────────────────────────────

struct TempDir(PathBuf);

impl TempDir {
    fn create() -> Result<Self> {
        let dir = store::home()
            .join(".rig/.tmp")
            .join(format!("install-{}", lock::now()));
        std::fs::create_dir_all(&dir)?;
        Ok(Self(dir))
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ── Git helpers ───────────────────────────────────────────────────────────────

fn git_available() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

fn clone(url: &str, git_ref: Option<&str>, dest: &Path) -> Result<()> {
    let is_sha = git_ref.map(|r| r.len() == 40 && r.chars().all(|c| c.is_ascii_hexdigit())).unwrap_or(false);

    if is_sha {
        // Commit SHA: clone default branch then checkout
        let status = Command::new("git")
            .args(["clone", "--no-checkout", url])
            .arg(dest)
            .status()?;
        if !status.success() {
            bail!("git clone failed");
        }
        let status = Command::new("git")
            .args(["checkout", git_ref.unwrap()])
            .current_dir(dest)
            .status()?;
        if !status.success() {
            bail!("git checkout {} failed", git_ref.unwrap());
        }
    } else {
        let mut cmd = Command::new("git");
        cmd.arg("clone").arg("--depth").arg("1");
        if let Some(r) = git_ref {
            cmd.args(["--branch", r]);
        }
        cmd.arg(url).arg(dest);
        let status = cmd.status()?;
        if !status.success() {
            bail!("git clone failed");
        }
    }
    Ok(())
}

fn clone_with_fallback(source: &ParsedSource, dest: &Path) -> Result<()> {
    let result = clone(&source.clone_url, source.git_ref.as_deref(), dest);
    if result.is_ok() {
        return Ok(());
    }
    // Fallback to SSH (private repos)
    let ssh_result = clone(&source.ssh_url, source.git_ref.as_deref(), dest);
    if ssh_result.is_ok() {
        return Ok(());
    }
    // Return the HTTPS error (more descriptive)
    result
}

fn get_commit_hash(dir: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()?;
    if out.status.success() {
        let hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if hash.len() == 40 {
            return Some(hash);
        }
    }
    None
}

pub fn get_remote_head(clone_url: &str, git_ref: Option<&str>) -> Option<String> {
    let refspec = git_ref.unwrap_or("HEAD");
    let out = Command::new("git")
        .args(["ls-remote", clone_url, refspec])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout.lines().next()?;
    let hash = line.split_whitespace().next()?;
    if hash.len() == 40 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(hash.to_string())
    } else {
        None
    }
}

// ── Skill copy + symlink ─────────────────────────────────────────────────────

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" {
            continue;
        }
        let s = entry.path();
        let d = dst.join(&name);
        if s.is_dir() {
            copy_dir(&s, &d)?;
        } else {
            std::fs::copy(&s, &d)?;
        }
    }
    Ok(())
}

fn symlink_skill(skill_name: &str, agent: &Agent, project_dir: Option<&PathBuf>) -> Result<()> {
    let store_path = store::skill_store().join(skill_name);
    let link_dir = agent.resolved_skill_dir(project_dir);
    std::fs::create_dir_all(&link_dir)?;
    let link = link_dir.join(skill_name);
    if link.symlink_metadata().is_ok() {
        let _ = std::fs::remove_file(&link).or_else(|_| std::fs::remove_dir_all(&link));
    }
    std::os::unix::fs::symlink(&store_path, &link)?;
    Ok(())
}

// ── Public install API ────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct InstallResult {
    pub skill_name: String,
    pub store_path: PathBuf,
    pub agents_linked: Vec<String>,
    pub commit: String,
}

/// Install a single skill from `source_dir` into the store and symlink to agents.
///
/// * `source_dir` – the directory containing `SKILL.md`
/// * `skill_name` – target name in the store (sanitised)
/// * `parsed`     – original source (for lock entry)
/// * `commit`     – commit hash of the cloned repo
/// * `subpath`    – relative path of the skill inside the repo
/// * `force`      – overwrite existing store entry
/// * `agents`     – which agents to symlink for
/// * `project_dir`– None = global, Some = project-scoped
pub fn do_install(
    source_dir: &Path,
    skill_name: &str,
    parsed: &ParsedSource,
    commit: &str,
    subpath: Option<&str>,
    force: bool,
    agents: &[&Agent],
    project_dir: Option<&PathBuf>,
) -> Result<InstallResult> {
    // Sanitise the name
    let skill_name = sanitise_name(skill_name)?;

    let store_path = store::skill_store().join(&skill_name);

    // Conflict check
    if store_path.exists() && !force {
        bail!(
            "Skill '{}' already exists in store. Use --force to overwrite.",
            skill_name
        );
    }

    // Atomic copy: write to temp path then rename
    let tmp = store_path.with_extension("__rig_tmp");
    if tmp.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    copy_dir(source_dir, &tmp)
        .context("Failed to copy skill to store")?;

    // Remove .git if it somehow ended up inside
    let _ = std::fs::remove_dir_all(tmp.join(".git"));

    // Verify SKILL.md survived the copy
    if !tmp.join("SKILL.md").exists() {
        let _ = std::fs::remove_dir_all(&tmp);
        bail!("SKILL.md missing after copy — aborting");
    }

    // Swap into place
    if store_path.exists() {
        let bak = store_path.with_extension("__rig_bak");
        let _ = std::fs::remove_dir_all(&bak);
        std::fs::rename(&store_path, &bak)?;
        if let Err(e) = std::fs::rename(&tmp, &store_path) {
            // Rollback
            let _ = std::fs::rename(&bak, &store_path);
            bail!("Swap failed: {e}");
        }
        let _ = std::fs::remove_dir_all(&bak);
    } else {
        std::fs::rename(&tmp, &store_path)?;
    }

    // Symlink to all requested agents
    let mut linked = Vec::new();
    for agent in agents {
        match symlink_skill(&skill_name, agent, project_dir) {
            Ok(_) => linked.push(agent.name.clone()),
            Err(e) => eprintln!(
                "  Warning: could not link to {}: {}",
                agent.name, e
            ),
        }
    }

    // Write lock entry
    lock::upsert(
        &skill_name,
        LockEntry {
            source: parsed.canonical_id(),
            commit: commit.to_string(),
            git_ref: parsed.git_ref.clone(),
            subpath: subpath.map(String::from),
            installed_at: lock::now(),
        },
    )?;

    Ok(InstallResult {
        skill_name,
        store_path,
        agents_linked: linked,
        commit: commit[..7.min(commit.len())].to_string(),
    })
}

fn sanitise_name(s: &str) -> Result<String> {
    if s.is_empty() {
        bail!("Skill name cannot be empty");
    }
    if s.contains("..") || s.contains('/') || s.contains('\\') || s.contains('\0') {
        bail!("Skill name contains unsafe characters: {s:?}");
    }
    if s.len() > 128 {
        bail!("Skill name is too long");
    }
    Ok(s.to_string())
}

// ── CLI: `rig install` ────────────────────────────────────────────────────────

pub struct InstallOpts<'a> {
    pub source: &'a str,
    /// None = all agents
    pub agent_filter: Option<&'a str>,
    /// None = global
    pub project_dir: Option<PathBuf>,
    pub force: bool,
    /// Install all skills without prompting
    pub all: bool,
    /// Skip security confirmation prompts
    pub yes: bool,
}

pub fn cmd_install(opts: &InstallOpts) -> Result<()> {
    if !git_available() {
        bail!("'git' is required for installing skills. Install git and try again.");
    }

    let config = store::load_config();
    let parsed = parse_source(opts.source)?;

    // Resolve which agents to install for
    let agents: Vec<&Agent> = match opts.agent_filter {
        None => config.agents.iter().collect(),
        Some("all") => config.agents.iter().collect(),
        Some(name) => {
            let found: Vec<_> = config
                .agents
                .iter()
                .filter(|a| a.name.to_lowercase() == name.to_lowercase())
                .collect();
            if found.is_empty() {
                let names: Vec<_> = config.agents.iter().map(|a| a.name.as_str()).collect();
                bail!("Unknown agent '{name}'. Available: {}", names.join(", "));
            }
            found
        }
    };

    // Ensure store exists
    std::fs::create_dir_all(store::skill_store())?;

    if parsed.is_local {
        let local_path = parsed.local_path.as_ref().unwrap();
        if !local_path.exists() {
            bail!("Local path does not exist: {}", local_path.display());
        }
        return install_local(&parsed, local_path, &agents, opts);
    }

    println!("  Cloning {}  {}…",
        parsed.owner, parsed.repo);

    let tmp = TempDir::create()?;
    clone_with_fallback(&parsed, tmp.path())
        .with_context(|| format!("Failed to clone {}", parsed.clone_url))?;

    // Get commit hash
    let commit = get_commit_hash(tmp.path()).unwrap_or_else(|| "unknown".into());

    // Find the root to search for skills (may be a subpath)
    let search_root = if let Some(sub) = &parsed.subpath {
        let p = tmp.path().join(sub);
        if !p.is_dir() {
            bail!("Subpath '{}' not found in repository", sub);
        }
        p
    } else {
        tmp.path().to_path_buf()
    };

    // Check if search_root itself is a skill (has SKILL.md at root)
    if search_root.join("SKILL.md").exists() {
        // Single-skill repo or direct subpath pointing at a skill
        let (name, _) = parse_skill_md(&search_root.join("SKILL.md"), &parsed.repo);
        return install_single(
            &parsed,
            &search_root,
            &name,
            parsed.subpath.as_deref(),
            &commit,
            &agents,
            opts,
        );
    }

    // Discover skills in the repo
    let skills = discover_skills(&search_root);

    if skills.is_empty() {
        bail!(
            "No skills found in {}. A skill directory must contain a SKILL.md file.",
            parsed.clone_url
        );
    }

    if skills.len() == 1 {
        let skill = &skills[0];
        let skill_dir = search_root.join(&skill.rel_path);
        return install_single(
            &parsed,
            &skill_dir,
            &skill.name,
            Some(&skill.rel_path),
            &commit,
            &agents,
            opts,
        );
    }

    // Multiple skills — present a picker
    println!();
    println!("  Found {} skills in {}/{}:", skills.len(), parsed.owner, parsed.repo);
    for (i, skill) in skills.iter().enumerate() {
        println!("    {:2}.  {:<24} {}", i + 1, skill.name, skill.description);
    }
    println!();

    let selected: Vec<usize> = if opts.all {
        (0..skills.len()).collect()
    } else {
        println!("  Install [all / 1,2,3 / q to cancel]: ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.eq_ignore_ascii_case("q") || input.eq_ignore_ascii_case("quit") {
            println!("  Cancelled.");
            return Ok(());
        }
        if input.eq_ignore_ascii_case("all") || input == "*" {
            (0..skills.len()).collect()
        } else {
            let mut indices = Vec::new();
            for part in input.split(',') {
                let part = part.trim();
                match part.parse::<usize>() {
                    Ok(n) if n >= 1 && n <= skills.len() => indices.push(n - 1),
                    _ => bail!("Invalid selection: {part:?}. Expected numbers 1-{}", skills.len()),
                }
            }
            indices
        }
    };

    for idx in selected {
        let skill = &skills[idx];
        let skill_dir = search_root.join(&skill.rel_path);
        install_single(
            &parsed,
            &skill_dir,
            &skill.name,
            Some(&skill.rel_path),
            &commit,
            &agents,
            opts,
        )?;
    }

    Ok(())
}

fn install_local(
    parsed: &ParsedSource,
    local_path: &Path,
    agents: &[&Agent],
    opts: &InstallOpts,
) -> Result<()> {
    // Check if it's a single skill or a collection
    if local_path.join("SKILL.md").exists() {
        let (name, _) = parse_skill_md(&local_path.join("SKILL.md"), &parsed.repo);
        run_security_check(&name, local_path, opts.yes)?;
        let result = do_install(local_path, &name, parsed, "local", None, opts.force, agents, opts.project_dir.as_ref())?;
        print_install_result(&result);
        return Ok(());
    }
    let skills = discover_skills(local_path);
    if skills.is_empty() {
        bail!("No SKILL.md files found under {}", local_path.display());
    }
    for skill in &skills {
        let dir = local_path.join(&skill.rel_path);
        run_security_check(&skill.name, &dir, opts.yes)?;
        let result = do_install(&dir, &skill.name, parsed, "local", Some(&skill.rel_path), opts.force, agents, opts.project_dir.as_ref())?;
        print_install_result(&result);
    }
    Ok(())
}

fn install_single(
    parsed: &ParsedSource,
    skill_dir: &Path,
    name: &str,
    subpath: Option<&str>,
    commit: &str,
    agents: &[&Agent],
    opts: &InstallOpts,
) -> Result<()> {
    run_security_check(name, skill_dir, opts.yes)?;
    let result = do_install(
        skill_dir,
        name,
        parsed,
        commit,
        subpath,
        opts.force,
        agents,
        opts.project_dir.as_ref(),
    )?;
    print_install_result(&result);
    Ok(())
}

fn run_security_check(name: &str, dir: &Path, yes: bool) -> Result<()> {
    let report = scanner::scan_dir(dir);
    scanner::print_report(name, &report);

    if report.is_clear() {
        return Ok(());
    }

    if yes {
        println!("  (--yes: proceeding despite security warnings)");
        return Ok(());
    }

    if report.verdict == scanner::Verdict::Dangerous {
        bail!(
            "Installation blocked: {}  Use --yes to override.",
            report.reason
        );
    }

    // Warning / Caution — ask
    print!("  Proceed with installation? [y/N]: ");
    use std::io::Write;
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if !input.trim().eq_ignore_ascii_case("y") {
        bail!("Cancelled.");
    }
    Ok(())
}

fn print_install_result(r: &InstallResult) {
    println!(
        "  \x1b[32m✓\x1b[0m  {} installed  ({})",
        r.skill_name,
        &r.commit
    );
    if !r.agents_linked.is_empty() {
        println!("     linked → {}", r.agents_linked.join(", "));
    }
}

// ── CLI: `rig outdated` ───────────────────────────────────────────────────────

#[derive(Debug)]
pub struct OutdatedEntry {
    pub name: String,
    pub installed: String,
    pub latest: String,
    pub source: String,
    pub is_outdated: bool,
    pub error: Option<String>,
}

pub fn cmd_outdated() -> Result<()> {
    let lock = lock::read();
    if lock.skills.is_empty() {
        println!("No skills tracked. Install skills with: rig install <source>");
        return Ok(());
    }

    println!("  Checking {} tracked skill(s)…\n", lock.skills.len());

    let mut entries: Vec<OutdatedEntry> = Vec::new();

    for (name, entry) in &lock.skills {
        if entry.source.starts_with("local:") {
            entries.push(OutdatedEntry {
                name: name.clone(),
                installed: short(&entry.commit),
                latest: short(&entry.commit),
                source: "local".into(),
                is_outdated: false,
                error: None,
            });
            continue;
        }

        // Derive clone URL from source string
        let clone_url = source_to_clone_url(&entry.source);
        if clone_url.is_none() {
            entries.push(OutdatedEntry {
                name: name.clone(),
                installed: short(&entry.commit),
                latest: "?".into(),
                source: entry.source.clone(),
                is_outdated: false,
                error: Some("Cannot determine remote URL".into()),
            });
            continue;
        }

        print!("  Checking {name}…");
        use std::io::Write;
        std::io::stdout().flush()?;

        let latest = get_remote_head(clone_url.as_deref().unwrap(), entry.git_ref.as_deref());
        match latest {
            None => {
                println!(" error");
                entries.push(OutdatedEntry {
                    name: name.clone(),
                    installed: short(&entry.commit),
                    latest: "?".into(),
                    source: entry.source.clone(),
                    is_outdated: false,
                    error: Some("Failed to fetch remote commit".into()),
                });
            }
            Some(ref latest_commit) => {
                let is_outdated = latest_commit != &entry.commit;
                if is_outdated {
                    println!(" outdated");
                } else {
                    println!(" up to date");
                }
                entries.push(OutdatedEntry {
                    name: name.clone(),
                    installed: short(&entry.commit),
                    latest: short(latest_commit),
                    source: entry.source.clone(),
                    is_outdated,
                    error: None,
                });
            }
        }
    }

    // Summary table
    println!();
    println!(
        "  {:<22} {:<10} {:<10} {}",
        "SKILL", "INSTALLED", "LATEST", "SOURCE"
    );
    println!("  {}", "─".repeat(60));

    let mut outdated_count = 0;
    for e in &entries {
        let installed_col = format!("{:<10}", e.installed);
        let latest_col = if e.is_outdated {
            format!("\x1b[31m{:<10}\x1b[0m", e.latest)
        } else if e.error.is_some() {
            format!("\x1b[2m{:<10}\x1b[0m", "error")
        } else {
            format!("\x1b[32m{:<10}\x1b[0m", e.latest)
        };
        let source = e.error.as_deref().unwrap_or(&e.source);
        println!("  {:<22} {installed_col} {latest_col} {}", e.name, source);
        if e.is_outdated {
            outdated_count += 1;
        }
    }

    println!();
    if outdated_count == 0 {
        println!("  \x1b[32mAll skills are up to date.\x1b[0m");
    } else {
        println!("  \x1b[33m{outdated_count} skill(s) outdated.\x1b[0m  Run: rig update");
    }

    Ok(())
}

fn source_to_clone_url(source: &str) -> Option<String> {
    if let Some(rest) = source.strip_prefix("github:") {
        // strip any #ref or :subpath
        let owner_repo = rest.split(&['#', ':'][..]).next()?;
        let slash = owner_repo.find('/')?;
        let owner = &owner_repo[..slash];
        let repo = &owner_repo[slash + 1..];
        Some(format!("https://github.com/{owner}/{repo}.git"))
    } else {
        None
    }
}

fn short(hash: &str) -> String {
    if hash.len() >= 7 {
        hash[..7].to_string()
    } else {
        hash.to_string()
    }
}

// ── CLI: `rig update` ─────────────────────────────────────────────────────────

pub fn cmd_update(names: &[String], yes: bool) -> Result<()> {
    if !git_available() {
        bail!("'git' is required for updating skills.");
    }

    let lock = lock::read();
    if lock.skills.is_empty() {
        println!("No skills tracked. Nothing to update.");
        return Ok(());
    }

    let config = store::load_config();

    let to_update: Vec<(&String, &lock::LockEntry)> = if names.is_empty() {
        lock.skills.iter().collect()
    } else {
        names
            .iter()
            .filter_map(|n| lock.skills.get_key_value(n))
            .collect()
    };

    if to_update.is_empty() {
        println!("No matching skills found in lock file.");
        return Ok(());
    }

    let mut updated = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;

    for (name, entry) in &to_update {
        println!("\n  Updating {}…", name);

        if entry.source.starts_with("local:") {
            println!("  Skipping (local skill — not updateable)");
            skipped += 1;
            continue;
        }

        let clone_url = match source_to_clone_url(&entry.source) {
            Some(u) => u,
            None => {
                eprintln!("  Error: cannot determine remote URL for '{name}'");
                failed += 1;
                continue;
            }
        };

        // Fetch latest commit
        let latest_commit = get_remote_head(&clone_url, entry.git_ref.as_deref());
        match &latest_commit {
            Some(c) if c == &entry.commit => {
                println!("  Already up to date ({})", short(&entry.commit));
                skipped += 1;
                continue;
            }
            None => {
                eprintln!("  Error: could not reach remote repository");
                failed += 1;
                continue;
            }
            _ => {}
        }

        // Clone new version
        let tmp = match TempDir::create() {
            Ok(t) => t,
            Err(e) => { eprintln!("  Error: {e}"); failed += 1; continue; }
        };

        // Build a ParsedSource just for the clone
        let source_parsed = match parse_source(&entry.source) {
            Ok(p) => p,
            Err(e) => { eprintln!("  Error parsing source: {e}"); failed += 1; continue; }
        };

        if let Err(e) = clone_with_fallback(&source_parsed, tmp.path()) {
            eprintln!("  Clone failed: {e}");
            failed += 1;
            continue;
        }

        let new_commit = get_commit_hash(tmp.path()).unwrap_or_else(|| "unknown".into());

        // Find skill dir inside clone
        let skill_dir = if let Some(sub) = &entry.subpath {
            tmp.path().join(sub)
        } else if tmp.path().join("SKILL.md").exists() {
            tmp.path().to_path_buf()
        } else {
            // Try to find the skill by name
            let skills = discover_skills(tmp.path());
            match skills.iter().find(|s| s.name == **name) {
                Some(s) => tmp.path().join(&s.rel_path),
                None => {
                    eprintln!("  Error: skill '{}' not found in new clone", name);
                    failed += 1;
                    continue;
                }
            }
        };

        if !skill_dir.exists() {
            eprintln!("  Error: skill directory not found in clone");
            failed += 1;
            continue;
        }

        // Security re-audit on the new version
        let report = scanner::scan_dir(&skill_dir);
        scanner::print_report(name, &report);

        if report.verdict == scanner::Verdict::Dangerous && !yes {
            println!("  Update blocked: {}  Use --yes to override.", report.reason);
            skipped += 1;
            continue;
        }

        if !report.is_clear() && !yes {
            print!("  Proceed with update? [y/N]: ");
            use std::io::Write;
            std::io::stdout().flush()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("  Skipped.");
                skipped += 1;
                continue;
            }
        }

        // Atomic swap
        let store_path = store::skill_store().join(name.as_str());
        let bak = store_path.with_extension("__rig_upd_bak");

        if store_path.exists() {
            if let Err(e) = std::fs::rename(&store_path, &bak) {
                eprintln!("  Error creating backup: {e}");
                failed += 1;
                continue;
            }
        }

        if let Err(e) = copy_dir(&skill_dir, &store_path) {
            eprintln!("  Error copying new version: {e}");
            let _ = std::fs::rename(&bak, &store_path);
            failed += 1;
            continue;
        }

        let _ = std::fs::remove_dir_all(store_path.join(".git"));
        let _ = std::fs::remove_dir_all(&bak);

        // Re-link to all agents (symlinks still point to store so no change needed
        // unless we want to ensure they exist)
        for agent in &config.agents {
            let _ = symlink_skill(name, agent, None);
        }

        // Update lock
        let mut new_entry = (*entry).clone();
        new_entry.commit = new_commit.clone();
        new_entry.installed_at = lock::now();
        let _ = lock::upsert(name, new_entry);

        println!(
            "  \x1b[32m✓\x1b[0m  {} → {}",
            short(&entry.commit),
            short(&new_commit)
        );
        updated += 1;
    }

    println!();
    println!(
        "  updated: {}  skipped: {}  failed: {}",
        updated, skipped, failed
    );
    Ok(())
}

// ── TUI: single-skill install (used from the TUI install mode) ────────────────

/// Minimal result for TUI display.
pub struct TuiInstallResult {
    pub installed: Vec<String>,
    pub error: Option<String>,
}

/// Run a quick install from the TUI (best-effort, no interactive prompts).
/// Always installs to global scope for all agents, with --yes and --all.
pub fn tui_install(source: &str, config: &RigConfig) -> TuiInstallResult {
    if !git_available() {
        return TuiInstallResult {
            installed: Vec::new(),
            error: Some("'git' is required for installing skills".into()),
        };
    }

    let agents: Vec<&Agent> = config.agents.iter().collect();
    let opts = InstallOpts {
        source,
        agent_filter: None,
        project_dir: None,
        force: false,
        all: true,
        yes: true, // skip security prompts in TUI
    };

    // We can't use cmd_install because it does I/O to stdin/stdout.
    // Run the logic directly.
    match tui_install_inner(source, &agents, &opts) {
        Ok(names) => TuiInstallResult { installed: names, error: None },
        Err(e) => TuiInstallResult { installed: Vec::new(), error: Some(e.to_string()) },
    }
}

fn tui_install_inner(source: &str, agents: &[&Agent], opts: &InstallOpts) -> Result<Vec<String>> {
    std::fs::create_dir_all(store::skill_store())?;

    let parsed = parse_source(source)?;
    let mut installed_names = Vec::new();

    if parsed.is_local {
        let local_path = parsed.local_path.as_ref().unwrap().clone();
        if !local_path.exists() {
            bail!("Path not found: {}", local_path.display());
        }
        if local_path.join("SKILL.md").exists() {
            let (name, _) = parse_skill_md(&local_path.join("SKILL.md"), &parsed.repo);
            let result = do_install(&local_path, &name, &parsed, "local", None, opts.force, agents, None)?;
            installed_names.push(result.skill_name);
        } else {
            for skill in discover_skills(&local_path) {
                let dir = local_path.join(&skill.rel_path);
                let result = do_install(&dir, &skill.name, &parsed, "local", Some(&skill.rel_path), opts.force, agents, None)?;
                installed_names.push(result.skill_name);
            }
        }
        return Ok(installed_names);
    }

    let tmp = TempDir::create()?;
    clone_with_fallback(&parsed, tmp.path())?;
    let commit = get_commit_hash(tmp.path()).unwrap_or_else(|| "unknown".into());

    let search_root = if let Some(sub) = &parsed.subpath {
        tmp.path().join(sub)
    } else {
        tmp.path().to_path_buf()
    };

    if search_root.join("SKILL.md").exists() {
        let (name, _) = parse_skill_md(&search_root.join("SKILL.md"), &parsed.repo);
        let result = do_install(&search_root, &name, &parsed, &commit, parsed.subpath.as_deref(), opts.force, agents, None)?;
        installed_names.push(result.skill_name);
    } else {
        let skills = discover_skills(&search_root);
        if skills.is_empty() {
            bail!("No skills found in {}", parsed.clone_url);
        }
        for skill in &skills {
            let dir = search_root.join(&skill.rel_path);
            let result = do_install(&dir, &skill.name, &parsed, &commit, Some(&skill.rel_path), opts.force, agents, None)?;
            installed_names.push(result.skill_name);
        }
    }

    Ok(installed_names)
}

// ── CLI help ──────────────────────────────────────────────────────────────────

pub fn print_install_help() {
    println!("rig install — install a skill from GitHub or a local path\n");
    println!("Usage:");
    println!("  rig install <source> [options]\n");
    println!("Sources:");
    println!("  owner/repo                     GitHub shorthand");
    println!("  github:owner/repo[#ref[:path]] Full GitHub source");
    println!("  https://github.com/owner/repo  GitHub URL");
    println!("  /path/to/skill                 Local directory\n");
    println!("Options:");
    println!("  --agent <name|all>   Install only for a specific agent (default: all)");
    println!("  --force              Overwrite existing skill in store");
    println!("  --all                Install all skills in a multi-skill repo");
    println!("  --yes                Skip security confirmation prompts\n");
    println!("Examples:");
    println!("  rig install anthropics/skills");
    println!("  rig install github:anthropics/skills --all");
    println!("  rig install https://github.com/obra/superpowers/tree/main/skills/commit");
    println!("  rig install ./my-skill --force");
}
