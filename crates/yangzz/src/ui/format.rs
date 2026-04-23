// ── ANSI escape codes — warm gold palette ──
//
//  Inspired by 薯条 SaaS dark theme: gold accent on dark background.
//  Primary accent: warm gold (256-color 178 ≈ #D4A843)
//  Secondary: soft green for success, red for errors, dim gray for secondary
//
pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const ITALIC: &str = "\x1b[3m";

// ── Brand colors (256-color for warm gold) ──
pub const GOLD: &str = "\x1b[38;5;178m";
pub const BOLD_GOLD: &str = "\x1b[1;38;5;178m";
pub const SOFT_GOLD: &str = "\x1b[38;5;179m";

// ── Standard ANSI (fallback / semantic) ──
pub const GREEN: &str = "\x1b[32m";
pub const RED: &str = "\x1b[31m";
pub const YELLOW: &str = "\x1b[33m";
pub const MAGENTA: &str = "\x1b[35m";
pub const BLUE: &str = "\x1b[34m";
pub const CYAN: &str = "\x1b[36m";
pub const WHITE: &str = "\x1b[37m";
pub const BOLD_GREEN: &str = "\x1b[1;32m";
pub const BOLD_RED: &str = "\x1b[1;31m";
pub const BOLD_YELLOW: &str = "\x1b[1;33m";
pub const BOLD_MAGENTA: &str = "\x1b[1;35m";
pub const BOLD_BLUE: &str = "\x1b[1;34m";
pub const BOLD_CYAN: &str = "\x1b[1;36m";
pub const BG_DARK: &str = "\x1b[48;5;236m";
pub const BG_INPUT: &str = "\x1b[48;5;235m";   // Slightly lighter dark bg for input line
pub const CLEAR_LINE: &str = "\x1b[2K";        // Clear entire line

// ── Semantic aliases (use these in UI code) ──
pub const ACCENT: &str = GOLD;
pub const BOLD_ACCENT: &str = BOLD_GOLD;
pub const PROMPT: &str = BOLD_GOLD;
pub const TOOL_COLOR: &str = SOFT_GOLD;
pub const SUCCESS: &str = GREEN;
pub const ERROR: &str = RED;
pub const INFO: &str = SOFT_GOLD;

/// Format duration in human-readable form
pub fn format_duration(secs: f64) -> String {
    if secs < 1.0 {
        format!("{:.0}ms", secs * 1000.0)
    } else if secs < 60.0 {
        format!("{:.1}s", secs)
    } else {
        let mins = (secs / 60.0).floor() as u64;
        let remaining = secs - (mins as f64 * 60.0);
        format!("{}m {:.0}s", mins, remaining)
    }
}

/// Format token count (1.2k, 3.5M etc.)
pub fn format_tokens(n: u32) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        format!("{n}")
    }
}

/// Format cost estimate
pub fn format_cost(input_tokens: u32, output_tokens: u32) -> String {
    // Rough estimate: $3/$15 per 1M tokens (Sonnet-class pricing)
    let cost = (input_tokens as f64 * 3.0 + output_tokens as f64 * 15.0) / 1_000_000.0;
    if cost < 0.01 {
        format!("<$0.01")
    } else {
        format!("${:.2}", cost)
    }
}
