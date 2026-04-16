use std::path::Path;

use crate::store;

/// Entry point for `rig migrate`
pub fn run() -> anyhow::Result<()> {
    let rig_home = store::home().join(".rig");
    let store = store::skill_store();

    println!("rig migrate — bringing skills under ~/.rig/\n");

    std::fs::create_dir_all(&store)?;

    // Step 1: Migrate config
    migrate_config(&rig_home)?;

    // Step 2: Load config
    let config = store::load_config();

    // Step 3: Migrate from ~/.agents/skills/
    let from_agents = migrate_from_agents(&store)?;

    // Step 4: Migrate skill-lock.json
    migrate_lock_file(&rig_home)?;

    // Step 5: Scan global agent dirs for loose skills
    let from_global = import_loose_skills(&config, &store, None)?;

    // Step 6: Scan managed project dirs for loose skills
    let mut from_projects = 0;
    for project in &config.projects {
        from_projects += import_loose_skills(&config, &store, Some(&project.path))?;
    }

    // Step 7: Repoint old symlinks (~/.agents/skills/ → ~/.rig/skills/)
    let repointed = repoint_old_symlinks(&config)?;

    // Summary
    println!();
    let total = from_agents + from_global + from_projects;
    if total == 0 && repointed == 0 {
        println!("Already up to date.");
    } else {
        if from_agents > 0 {
            println!("  {} skills from ~/.agents/skills/", from_agents);
        }
        if from_global > 0 {
            println!("  {} loose skills from global agent dirs", from_global);
        }
        if from_projects > 0 {
            println!("  {} loose skills from {} managed projects", from_projects, config.projects.len());
        }
        if repointed > 0 {
            println!("  {} symlinks repointed", repointed);
        }
        println!("\nDone! Run `rig` to manage your skills.");
    }

    Ok(())
}

fn migrate_config(rig_home: &Path) -> anyhow::Result<()> {
    let old = store::home().join(".config/rig/config.json");
    let new = rig_home.join("config.json");

    if old.exists() && !new.exists() {
        move_path(&old, &new)?;
        let _ = std::fs::remove_dir(old.parent().unwrap());
        println!("  Config → ~/.rig/config.json");
    }
    Ok(())
}

fn migrate_from_agents(store: &Path) -> anyhow::Result<usize> {
    let old_store = store::home().join(".agents/skills");
    if !old_store.is_dir() {
        return Ok(0);
    }

    let entries: Vec<_> = std::fs::read_dir(&old_store)?
        .flatten()
        .filter(|e| e.path().is_dir())
        .collect();

    if entries.is_empty() {
        return Ok(0);
    }

    println!("  Found {} skills in ~/.agents/skills/", entries.len());

    let mut count = 0;
    for entry in entries {
        let old_path = entry.path();
        let new_path = store.join(entry.file_name());

        if new_path.exists() {
            continue;
        }

        move_path(&old_path, &new_path)?;
        count += 1;
    }

    let _ = std::fs::remove_dir(&old_store);
    let _ = std::fs::remove_dir(old_store.parent().unwrap());

    Ok(count)
}

fn migrate_lock_file(rig_home: &Path) -> anyhow::Result<()> {
    let old = store::home().join(".agents/.skill-lock.json");
    let new = rig_home.join("skill-lock.json");

    if old.exists() && !new.exists() {
        std::fs::copy(&old, &new)?;
        println!("  Copied skill-lock.json");
    }
    Ok(())
}

/// Scan agent dirs for real (non-symlink) skill dirs, import to store.
/// project_dir=None → global, Some → project-level.
fn import_loose_skills(
    config: &store::RigConfig,
    store: &Path,
    project_dir: Option<&std::path::PathBuf>,
) -> anyhow::Result<usize> {
    let mut count = 0;

    for agent in &config.agents {
        let agent_dir = agent.resolved_skill_dir(project_dir);
        let Ok(entries) = std::fs::read_dir(&agent_dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if name.starts_with('.') || !path.is_dir() || !path.join("SKILL.md").exists() {
                continue;
            }

            // Already a symlink — already managed
            if path.symlink_metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
            {
                continue;
            }

            let store_path = store.join(&name);
            if !store_path.exists() {
                move_path(&path, &store_path)?;
                count += 1;
            } else {
                std::fs::remove_dir_all(&path)?;
            }

            // Replace with symlink
            std::os::unix::fs::symlink(&store_path, &path)?;

            let scope = match project_dir {
                Some(p) => p.file_name().unwrap_or_default().to_string_lossy().to_string(),
                None => "global".to_string(),
            };
            println!("    {} ← {} ({})", name, agent.name, scope);
        }
    }

    Ok(count)
}

/// Repoint symlinks that still target ~/.agents/skills/ → ~/.rig/skills/
fn repoint_old_symlinks(config: &store::RigConfig) -> anyhow::Result<usize> {
    let old_store = store::home().join(".agents/skills");
    let new_store = store::skill_store();
    let mut count = 0;

    for agent in &config.agents {
        count += repoint_in_dir(&agent.resolved_skill_dir(None), &old_store, &new_store);
        for project in &config.projects {
            count += repoint_in_dir(
                &agent.resolved_skill_dir(Some(&project.path)),
                &old_store,
                &new_store,
            );
        }
    }

    Ok(count)
}

fn repoint_in_dir(dir: &Path, old_store: &Path, new_store: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    let mut count = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = path.symlink_metadata() else { continue };
        if !meta.file_type().is_symlink() {
            continue;
        }
        let Ok(target) = std::fs::read_link(&path) else { continue };
        if target.starts_with(old_store) {
            let new_target = new_store.join(target.file_name().unwrap_or_default());
            if new_target.exists() {
                let _ = std::fs::remove_file(&path);
                let _ = std::os::unix::fs::symlink(&new_target, &path);
                count += 1;
            }
        }
    }
    count
}

fn move_path(src: &Path, dst: &Path) -> anyhow::Result<()> {
    if std::fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    if src.is_dir() {
        copy_dir_recursive(src, dst)?;
        std::fs::remove_dir_all(src)?;
    } else {
        std::fs::copy(src, dst)?;
        std::fs::remove_file(src)?;
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let (s, d) = (entry.path(), dst.join(entry.file_name()));
        if s.is_dir() {
            copy_dir_recursive(&s, &d)?;
        } else {
            std::fs::copy(&s, &d)?;
        }
    }
    Ok(())
}
