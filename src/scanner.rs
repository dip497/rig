/// Security scanner — ported from ASM's security-auditor.ts.
///
/// Scans a skill directory for dangerous patterns before install or update.
use std::path::Path;

// ── Pattern table ────────────────────────────────────────────────────────────

/// Raw pattern descriptor. All fields are Copy so this can live in a static.
#[derive(Clone, Copy)]
struct P {
    category: &'static str,
    needle: &'static str,
    /// 0 = critical, 1 = warning, 2 = info
    sev: u8,
    /// Contributes to shell permission flag
    shell: bool,
    /// Contributes to network permission flag
    network: bool,
    /// Contributes to code-execution permission flag
    code_exec: bool,
}

const PATTERNS: &[P] = &[
    // ── Network ──────────────────────────────────────────────────────────────
    P { category: "Network", needle: "curl ",        sev: 0, shell: false, network: true,  code_exec: false },
    P { category: "Network", needle: "wget ",        sev: 0, shell: false, network: true,  code_exec: false },
    P { category: "Network", needle: "fetch(",       sev: 1, shell: false, network: true,  code_exec: false },
    P { category: "Network", needle: "axios",        sev: 1, shell: false, network: true,  code_exec: false },
    P { category: "Network", needle: "http.request", sev: 1, shell: false, network: true,  code_exec: false },
    P { category: "Network", needle: "XMLHttpRequest", sev: 1, shell: false, network: true, code_exec: false },
    P { category: "Network", needle: "http://",      sev: 1, shell: false, network: true,  code_exec: false },
    P { category: "Network", needle: "https://",     sev: 2, shell: false, network: true,  code_exec: false },
    // ── Shell execution ───────────────────────────────────────────────────────
    P { category: "Shell", needle: "exec(",          sev: 0, shell: true,  network: false, code_exec: false },
    P { category: "Shell", needle: "execSync",       sev: 0, shell: true,  network: false, code_exec: false },
    P { category: "Shell", needle: "child_process",  sev: 0, shell: true,  network: false, code_exec: false },
    P { category: "Shell", needle: "spawn(",         sev: 0, shell: true,  network: false, code_exec: false },
    P { category: "Shell", needle: "bash -c",        sev: 0, shell: true,  network: false, code_exec: false },
    P { category: "Shell", needle: "sh -c",          sev: 0, shell: true,  network: false, code_exec: false },
    P { category: "Shell", needle: "Bun.spawn",      sev: 0, shell: true,  network: false, code_exec: false },
    // ── Dynamic code execution ────────────────────────────────────────────────
    P { category: "Code execution", needle: "eval(",        sev: 0, shell: false, network: false, code_exec: true },
    P { category: "Code execution", needle: "new Function(", sev: 0, shell: false, network: false, code_exec: true },
    P { category: "Code execution", needle: "Function(",    sev: 1, shell: false, network: false, code_exec: true },
    // ── Embedded credentials ──────────────────────────────────────────────────
    P { category: "Credentials", needle: "API_KEY=",      sev: 0, shell: false, network: false, code_exec: false },
    P { category: "Credentials", needle: "SECRET_KEY=",   sev: 0, shell: false, network: false, code_exec: false },
    P { category: "Credentials", needle: "PASSWORD=",     sev: 0, shell: false, network: false, code_exec: false },
    P { category: "Credentials", needle: "ACCESS_TOKEN=", sev: 0, shell: false, network: false, code_exec: false },
    P { category: "Credentials", needle: "PRIVATE_KEY=",  sev: 0, shell: false, network: false, code_exec: false },
    // ── Obfuscation ───────────────────────────────────────────────────────────
    P { category: "Obfuscation", needle: "atob(",       sev: 1, shell: false, network: false, code_exec: false },
    P { category: "Obfuscation", needle: "Buffer.from(", sev: 2, shell: false, network: false, code_exec: false },
    // ── Dangerous filesystem ops ───────────────────────────────────────────────
    P { category: "Filesystem", needle: "rm -rf",    sev: 0, shell: true,  network: false, code_exec: false },
    P { category: "Filesystem", needle: "writeFile", sev: 2, shell: false, network: false, code_exec: false },
    P { category: "Filesystem", needle: "unlink(",   sev: 1, shell: false, network: false, code_exec: false },
];

// ── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Critical => write!(f, "critical"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScanMatch {
    pub file: String,
    pub line: usize,
    pub text: String,
    pub severity: Severity,
    pub category: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Safe,
    Caution,
    Warning,
    Dangerous,
}

impl std::fmt::Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Verdict::Safe => write!(f, "SAFE"),
            Verdict::Caution => write!(f, "CAUTION"),
            Verdict::Warning => write!(f, "WARNING"),
            Verdict::Dangerous => write!(f, "DANGEROUS"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScanReport {
    pub matches: Vec<ScanMatch>,
    pub verdict: Verdict,
    pub reason: String,
    pub file_count: usize,
    pub line_count: usize,
}

impl ScanReport {
    pub fn critical_count(&self) -> usize {
        self.matches.iter().filter(|m| m.severity == Severity::Critical).count()
    }
    pub fn warning_count(&self) -> usize {
        self.matches.iter().filter(|m| m.severity == Severity::Warning).count()
    }
    /// True if safe to proceed without explicit override.
    pub fn is_clear(&self) -> bool {
        matches!(self.verdict, Verdict::Safe | Verdict::Caution)
    }
}

// ── Scanner ───────────────────────────────────────────────────────────────────

pub fn scan_dir(dir: &Path) -> ScanReport {
    let mut matches: Vec<ScanMatch> = Vec::new();
    let mut file_count = 0usize;
    let mut line_count = 0usize;

    scan_recursive(dir, dir, &mut matches, &mut file_count, &mut line_count);

    let has_shell = matches.iter().any(|m| {
        PATTERNS.iter().any(|p| p.shell && p.category == m.category)
    });
    let has_network = matches.iter().any(|m| {
        PATTERNS.iter().any(|p| p.network && p.category == m.category)
    });
    let has_code_exec = matches.iter().any(|m| {
        PATTERNS.iter().any(|p| p.code_exec && p.category == m.category)
    });

    let crit = matches.iter().filter(|m| m.severity == Severity::Critical).count();
    let warn = matches.iter().filter(|m| m.severity == Severity::Warning).count();

    let (verdict, reason) = if has_shell && has_network {
        (
            Verdict::Dangerous,
            "Has both shell execution and network access — potential data exfiltration.".into(),
        )
    } else if has_code_exec && has_network {
        (
            Verdict::Dangerous,
            "Has dynamic code execution and network access — potential remote code execution.".into(),
        )
    } else if crit >= 10 {
        (Verdict::Dangerous, format!("{crit} critical findings — high concentration of risky patterns."))
    } else if has_shell || has_code_exec {
        let r = if has_shell {
            "Executes shell commands — review carefully before installing."
        } else {
            "Uses dynamic code execution — review carefully."
        };
        (Verdict::Warning, r.into())
    } else if crit > 0 {
        (
            Verdict::Warning,
            format!("{crit} critical finding(s) detected — manual review recommended."),
        )
    } else if warn > 0 {
        (
            Verdict::Caution,
            format!("{warn} warning(s) found — generally acceptable but worth reviewing."),
        )
    } else {
        (Verdict::Safe, "No suspicious patterns detected.".into())
    };

    ScanReport { matches, verdict, reason, file_count, line_count }
}

fn scan_recursive(
    base: &Path,
    dir: &Path,
    matches: &mut Vec<ScanMatch>,
    file_count: &mut usize,
    line_count: &mut usize,
) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            scan_recursive(base, &path, matches, file_count, line_count);
            continue;
        }
        if !path.is_file() {
            continue;
        }
        *file_count += 1;
        // Only scan text-ish files; skip large binaries
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if content.len() > 1_000_000 {
            continue; // skip files > 1 MB
        }
        let rel = path
            .strip_prefix(base)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        for (line_no, line) in content.lines().enumerate() {
            *line_count += 1;
            for pat in PATTERNS {
                if line.contains(pat.needle) {
                    let text = line.trim().to_string();
                    let text = if text.len() > 100 {
                        format!("{}…", &text[..100])
                    } else {
                        text
                    };
                    let severity = match pat.sev {
                        0 => Severity::Critical,
                        1 => Severity::Warning,
                        _ => Severity::Info,
                    };
                    matches.push(ScanMatch {
                        file: rel.clone(),
                        line: line_no + 1,
                        text,
                        severity,
                        category: pat.category,
                    });
                    break; // one match per line is enough
                }
            }
        }
    }
}

// ── Report formatting (for CLI output) ───────────────────────────────────────

pub fn print_report(name: &str, report: &ScanReport) {
    let verdict_str = match report.verdict {
        Verdict::Safe => "\x1b[32m[ SAFE ]\x1b[0m",
        Verdict::Caution => "\x1b[36m[ CAUTION ]\x1b[0m",
        Verdict::Warning => "\x1b[33m[ WARNING ]\x1b[0m",
        Verdict::Dangerous => "\x1b[31m[ DANGEROUS ]\x1b[0m",
    };

    println!();
    println!("  Security scan: {name}  {verdict_str}");
    println!("  {} files  {} lines", report.file_count, report.line_count);

    if report.matches.is_empty() {
        println!("  \x1b[32m✓\x1b[0m  No suspicious patterns detected.");
        return;
    }

    println!("  {}", report.reason);

    if report.critical_count() > 0 || report.warning_count() > 0 {
        println!();
        // Group by category
        let mut seen_categories: Vec<&str> = Vec::new();
        let mut ordered_matches: Vec<&ScanMatch> = Vec::new();
        for m in &report.matches {
            if !seen_categories.contains(&m.category) {
                seen_categories.push(m.category);
            }
        }
        for cat in &seen_categories {
            let cat_matches: Vec<_> = report.matches.iter().filter(|m| &m.category == cat).collect();
            let sev_label = match cat_matches[0].severity {
                Severity::Critical => "\x1b[31m!!\x1b[0m",
                Severity::Warning  => "\x1b[33m !\x1b[0m",
                Severity::Info     => "\x1b[2m i\x1b[0m",
            };
            println!("  {} {cat} ({} matches)", sev_label, cat_matches.len());
            for m in cat_matches.iter().take(3) {
                println!("      \x1b[2m{}:{}\x1b[0m  {}", m.file, m.line, m.text);
            }
            if cat_matches.len() > 3 {
                println!("      \x1b[2m… {} more\x1b[0m", cat_matches.len() - 3);
            }
            ordered_matches.extend(cat_matches);
        }
    }
    println!();
}
