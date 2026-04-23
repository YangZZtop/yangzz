use std::path::{Path, PathBuf};

const MEMORY_FILE: &str = "MEMORY.md";
const MAX_MEMORY_BYTES: usize = 32 * 1024; // 32KB max

// ── 4-Layer Memory Stack ──
// L0: Full detail (MEMORY.md raw) — used when context budget is ample
// L1: Summary (auto-generated summaries) — used at 50-80% budget
// L2: Keywords only — used at >80% budget
// L3: None — budget exhausted, no memory injected

/// Memory budget level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryLevel {
    L0Full,      // Full MEMORY.md (< 50% context used)
    L1Summary,   // Summarized entries (50-80%)
    L2Keywords,  // Keywords only (80-95%)
    L3None,      // No memory injected (> 95%)
}

impl MemoryLevel {
    /// Determine memory level from context usage ratio (0.0 - 1.0)
    pub fn from_usage(ratio: f64) -> Self {
        if ratio < 0.50 { MemoryLevel::L0Full }
        else if ratio < 0.80 { MemoryLevel::L1Summary }
        else if ratio < 0.95 { MemoryLevel::L2Keywords }
        else { MemoryLevel::L3None }
    }

    pub fn label(&self) -> &'static str {
        match self {
            MemoryLevel::L0Full => "L0:Full",
            MemoryLevel::L1Summary => "L1:Summary",
            MemoryLevel::L2Keywords => "L2:Keywords",
            MemoryLevel::L3None => "L3:Off",
        }
    }
}

/// Load MEMORY.md from project root or global config
pub fn load_memory(cwd: &Path) -> Option<String> {
    // 1. Try project-local MEMORY.md
    let local = cwd.join(MEMORY_FILE);
    if local.exists() {
        if let Ok(content) = std::fs::read_to_string(&local) {
            return Some(content);
        }
    }

    // 2. Try global MEMORY.md
    let global = global_memory_path();
    if global.exists() {
        if let Ok(content) = std::fs::read_to_string(&global) {
            return Some(content);
        }
    }

    None
}

/// Append an observation to MEMORY.md (project-local)
pub fn append_memory(cwd: &Path, entry: &str) -> Result<(), String> {
    let path = cwd.join(MEMORY_FILE);
    let mut content = if path.exists() {
        std::fs::read_to_string(&path).unwrap_or_default()
    } else {
        "# Project Memory\n\n".to_string()
    };

    // Dedup: check if this exact entry already exists
    if content.contains(entry.trim()) {
        return Ok(());
    }

    // Check size limit
    if content.len() + entry.len() > MAX_MEMORY_BYTES {
        return Err("MEMORY.md size limit reached (32KB)".into());
    }

    content.push_str("\n- ");
    content.push_str(entry.trim());
    content.push('\n');

    std::fs::write(&path, content).map_err(|e| format!("Cannot write MEMORY.md: {e}"))
}

/// Global memory path
fn global_memory_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("yangzz")
        .join(MEMORY_FILE)
}

/// Inject memory into system prompt with budget-aware level degradation
pub fn inject_memory_prompt(system: &str, cwd: &Path) -> String {
    inject_memory_at_level(system, cwd, MemoryLevel::L0Full)
}

/// Inject memory at a specific budget level
pub fn inject_memory_at_level(system: &str, cwd: &Path, level: MemoryLevel) -> String {
    if level == MemoryLevel::L3None {
        return system.to_string();
    }

    let mem = match load_memory(cwd) {
        Some(m) if !m.trim().is_empty() => m,
        _ => return system.to_string(),
    };

    let injected = match level {
        MemoryLevel::L0Full => {
            // Full content, capped at 4KB
            if mem.len() > 4096 {
                format!("{}... (truncated)", &mem[..4096])
            } else {
                mem
            }
        }
        MemoryLevel::L1Summary => {
            // Summarize: take first line of each bullet point, cap at 1KB
            let summary: String = mem
                .lines()
                .filter(|l| l.starts_with("- ") || l.starts_with("# "))
                .map(|l| {
                    if l.len() > 80 { format!("{}..", &l[..80]) } else { l.to_string() }
                })
                .collect::<Vec<_>>()
                .join("\n");
            if summary.len() > 1024 {
                format!("{}...", &summary[..1024])
            } else {
                summary
            }
        }
        MemoryLevel::L2Keywords => {
            // Extract only key terms, max 256 chars
            let words: Vec<&str> = mem
                .split_whitespace()
                .filter(|w| w.len() > 3 && !w.starts_with('#') && !w.starts_with('-'))
                .take(30)
                .collect();
            format!("Key context: {}", words.join(", "))
        }
        MemoryLevel::L3None => unreachable!(),
    };

    format!(
        "{system}\n\n--- Project Memory [{level}] ---\n{injected}",
        level = level.label()
    )
}

// ── Hermes: Self-Evolution Loop ──
// Automatically detects user patterns and writes preferences to MEMORY.md

/// Hermes pattern detectors
pub fn hermes_analyze(user_input: &str, assistant_output: &str, cwd: &Path) {
    let mut observations = Vec::new();

    // Detect language preference
    let has_cjk = user_input.chars().any(|c| c >= '\u{4E00}' && c <= '\u{9FFF}');
    if has_cjk {
        observations.push("User prefers Chinese responses");
    }

    // Detect coding style preferences from corrections
    let corrections = [
        ("不要加注释", "User prefers code without comments"),
        ("用中文", "User wants Chinese language responses"),
        ("不要解释", "User prefers direct action over explanations"),
        ("一次性做完", "User wants complete implementations, not incremental"),
        ("don't explain", "User prefers direct action over explanations"),
        ("no comments", "User prefers code without comments"),
    ];
    for (pattern, observation) in &corrections {
        if user_input.to_lowercase().contains(*pattern) {
            observations.push(observation);
        }
    }

    // Detect framework/tool preferences from context
    let tech_patterns = [
        ("react", "Project uses React"),
        ("vue", "Project uses Vue"),
        ("nextjs", "Project uses Next.js"),
        ("tailwind", "Project uses Tailwind CSS"),
        ("typescript", "Project uses TypeScript"),
        ("rust", "Project uses Rust"),
        ("python", "Project uses Python"),
        ("cargo", "Project uses Cargo/Rust"),
        ("npm", "Project uses npm/Node.js"),
    ];
    for (pattern, observation) in &tech_patterns {
        if assistant_output.to_lowercase().contains(pattern) {
            observations.push(observation);
        }
    }

    // Write unique observations to MEMORY.md
    for obs in observations {
        let _ = append_memory(cwd, obs);
    }
}

/// Detect frustration patterns and return strategy hint
pub fn detect_frustration(user_input: &str) -> Option<&'static str> {
    let lower = user_input.to_lowercase();
    let frustration_signals = [
        "不是这样", "错了", "不对", "重来", "再试", "为什么又",
        "都说了", "你没听", "搞什么", "不行", "废话",
        "wrong", "no!", "redo", "try again", "not what i asked",
        "that's wrong", "you broke", "fix this",
    ];

    for signal in &frustration_signals {
        if lower.contains(signal) {
            return Some(
                "[STRATEGY SHIFT: The user seems frustrated. Be more careful, ask for clarification before acting, and double-check your work. Show your reasoning step by step.]"
            );
        }
    }
    None
}
