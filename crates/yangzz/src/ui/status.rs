use super::banner;
use super::format::*;
use super::i18n::t;
use crate::emitln;
use std::process::Command;
use yangzz_core::config::model_meta;

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
        self.total_cost_usd += (input as f64 * ip + output as f64 * op) / 1_000_000.0;
    }
}

// Status bar internals: force a dark background so neon colors are readable
// regardless of whether the user's terminal is dark or light themed.
//   BG     — set background to near-black
//   FG_DEF — reset only the foreground (keeps BG intact across segments)
const BG: &str = "\x1b[48;5;235m"; // bg color: #262626
const FG_DEF: &str = "\x1b[39m"; // reset fg only (preserves bg)
// Separator: bright violet pipe. Uses FG_DEF (not RESET) so the BG carries over.
const SEP: &str = " \x1b[38;5;141m│\x1b[39m ";

/// Render a high-contrast status line:  dir │ branch │ model │ tokens │ cost │ time
///
/// The inner content is painted on a forced dark background so the neon
/// foreground colors always pop — whether the user's terminal is light or
/// dark themed.
pub fn render_status_bar(stats: &SessionStats) {
    for line in status_bar_lines(stats) {
        println!("{line}");
    }
}

pub fn emit_status_bar(stats: &SessionStats) {
    for line in status_bar_lines(stats) {
        emitln!("{line}");
    }
}

fn status_bar_lines(stats: &SessionStats) -> Vec<String> {
    let git = get_git_info();
    let total = stats.total_input_tokens + stats.total_output_tokens;

    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let short_dir = short_dirname(&cwd);

    let mut parts = Vec::new();

    // Each field: set fg color, then FG_DEF (keeps bg). Never use RESET inside.

    // Directory — neon cyan
    parts.push(format!("{BOLD_NEON_CYAN}{short_dir}{FG_DEF}"));

    // Git branch + sync — neon green
    if let Some((branch, dirty)) = &git {
        let mark = if *dirty { "*" } else { "" };
        parts.push(format!("{BOLD_NEON_GREEN}{branch}{mark}{FG_DEF}"));
    }

    // Model — neon pink (signature)
    parts.push(format!(
        "{BOLD_NEON_PINK}{}{FG_DEF}",
        friendly_model(&stats.model)
    ));

    // Tokens — neon yellow (now readable on any bg thanks to forced dark bg)
    if total > 0 {
        parts.push(format!(
            "{BOLD_NEON_YELLOW}{}{FG_DEF} {NEON_YELLOW}{}{FG_DEF}",
            format_tokens(total),
            t().tokens_label
        ));
    } else {
        parts.push(format!("{NEON_YELLOW}0 {}{FG_DEF}", t().tokens_label));
    }

    // Cost — neon orange
    if stats.total_cost_usd > 0.0001 {
        parts.push(format!(
            "{BOLD_NEON_ORANGE}${:.4}{FG_DEF}",
            stats.total_cost_usd
        ));
    }

    // Time — neon blue
    let now = chrono::Local::now();
    parts.push(format!("{NEON_BLUE}{}{FG_DEF}", now.format("%H:%M")));

    let bar = parts.join(SEP);
    let term_w = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);
    let bw = term_w.min(62).max(40);
    // Compute visual width of the bar (strip ANSI) to pad to box inner width
    let visual_width = strip_ansi_width(&bar);
    let inner_pad_total = (bw.saturating_sub(visual_width + 1)).max(0); // -1 for leading space
    let padding = " ".repeat(inner_pad_total);

    // Bright violet frame so the status bar actually pops off the scrollback.
    // Fill the bar line with BG all the way to the right border.
    let hr: String = "─".repeat(bw);
    vec![
        String::new(),
        format!("  {NEON_VIOLET}╭{hr}╮{RESET}"),
        format!("  {NEON_VIOLET}│{BG} {bar}{padding}{RESET}{NEON_VIOLET}│{RESET}"),
        format!("  {NEON_VIOLET}╰{hr}╯{RESET}"),
    ]
}

/// Compute visual (column) width of a string with ANSI escape codes stripped,
/// treating CJK chars as width 2.
fn strip_ansi_width(s: &str) -> usize {
    let mut width = 0usize;
    let mut in_esc = false;
    for ch in s.chars() {
        if ch == '\x1b' {
            in_esc = true;
            continue;
        }
        if in_esc {
            if ch == 'm' {
                in_esc = false;
            }
            continue;
        }
        if ('\u{4e00}'..='\u{9fff}').contains(&ch)
            || ('\u{3400}'..='\u{4dbf}').contains(&ch)
            || ('\u{ff00}'..='\u{ffef}').contains(&ch)
        {
            width += 2;
        } else {
            width += 1;
        }
    }
    width
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
