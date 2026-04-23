use super::banner;
use super::format::*;
use super::i18n::t;
use yangzz_core::config::model_meta;
use std::process::Command;

/// Session-level stats tracked across turns
pub struct SessionStats {
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub total_turns: u32,
    pub total_cost_usd: f64,
    pub model: String,
    pub provider: String,
}

impl SessionStats {
    pub fn new(model: &str, provider: &str) -> Self {
        Self {
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_turns: 0,
            total_cost_usd: 0.0,
            model: model.to_string(),
            provider: provider.to_string(),
        }
    }

    pub fn add_usage(&mut self, input: u32, output: u32) {
        self.total_input_tokens += input;
        self.total_output_tokens += output;
        self.total_turns += 1;
        // Use real model pricing if available, fallback to generic estimate
        let (ip, op) = model_meta::lookup_model(&self.model)
            .map(|m| (m.input_price, m.output_price))
            .unwrap_or((3.0, 15.0));
        self.total_cost_usd +=
            (input as f64 * ip + output as f64 * op) / 1_000_000.0;
    }
}

const SEP: &str = " \x1b[2m│\x1b[0m ";

/// Render LegnaCode-style status line:  dir │ branch │ model │ tokens │ cost │ time
pub fn render_status_bar(stats: &SessionStats) {
    let git = get_git_info();
    let total = stats.total_input_tokens + stats.total_output_tokens;

    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let short_dir = short_dirname(&cwd);

    let mut parts = Vec::new();

    // Directory
    parts.push(format!("{SOFT_GOLD}{short_dir}{RESET}"));

    // Git branch + sync
    if let Some((branch, dirty)) = &git {
        let mark = if *dirty { "*" } else { "" };
        parts.push(format!("{GREEN}{BOLD}{branch}{mark}{RESET}"));
    }

    // Model
    parts.push(format!("{GOLD}{}{RESET}", friendly_model(&stats.model)));

    // Tokens
    if total > 0 {
        parts.push(format!("{DIM}{} {}{RESET}", format_tokens(total), t().tokens_label));
    } else {
        parts.push(format!("{DIM}0 {}{RESET}", t().tokens_label));
    }

    // Cost
    if stats.total_cost_usd > 0.0001 {
        parts.push(format!("{GREEN}${:.4}{RESET}", stats.total_cost_usd));
    }

    // Time
    let now = chrono::Local::now();
    parts.push(format!("{DIM}{}{RESET}", now.format("%H:%M")));

    let bar = parts.join(SEP);
    let term_w = crossterm::terminal::size().map(|(w, _)| w as usize).unwrap_or(80);
    let bw = term_w.min(62).max(40);
    // Box around status bar
    let hr: String = "─".repeat(bw);
    println!("  {DIM}╭{hr}╮{RESET}");
    println!("  {DIM}│{RESET} {bar}{RESET}");
    println!("  {DIM}╰{hr}╯{RESET}");
}

/// Render timing after a turn (Claude Code "Cogitated for X" style)
pub fn render_turn_info(duration_secs: f64) {
    let dur = format_duration(duration_secs);
    println!();
    println!("  {DIM}{} {dur}{RESET}", t().cooked_for);
}

/// Friendly model name: claude-sonnet-4-20250514 → Sonnet 4
fn friendly_model(id: &str) -> String {
    let lo = id.to_lowercase();
    // claude-opus-4-..., claude-sonnet-4-5-...
    if let Some(caps) = lo.strip_prefix("claude-") {
        let parts: Vec<&str> = caps.split('-').collect();
        if !parts.is_empty() {
            let family = capitalize(parts[0]);
            let ver = parts.get(1).unwrap_or(&"");
            return format!("{family} {ver}");
        }
    }
    // Already short enough
    if id.len() <= 20 {
        id.to_string()
    } else {
        format!("{}...", &id[..17])
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn short_dirname(cwd: &str) -> String {
    let short = banner::shorten_path(cwd);
    // Only show last directory component for brevity
    short.rsplit('/').next().unwrap_or(&short).to_string()
}

fn get_git_info() -> Option<(String, bool)> {
    let branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())?;

    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    Some((branch, dirty))
}
