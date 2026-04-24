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

// ── Neon palette (256-color high-saturation) ──
// Used for role differentiation and visual punch in status bar / dividers.
pub const NEON_PINK: &str = "\x1b[38;5;201m"; // #FF00FF — user
pub const BOLD_NEON_PINK: &str = "\x1b[1;38;5;201m";
pub const NEON_CYAN: &str = "\x1b[38;5;51m"; // #00FFFF — assistant
pub const BOLD_NEON_CYAN: &str = "\x1b[1;38;5;51m";
pub const NEON_GREEN: &str = "\x1b[38;5;46m"; // #00FF00 — success/tool
pub const BOLD_NEON_GREEN: &str = "\x1b[1;38;5;46m";
pub const NEON_YELLOW: &str = "\x1b[38;5;226m"; // #FFFF00 — info
pub const BOLD_NEON_YELLOW: &str = "\x1b[1;38;5;226m";
pub const NEON_ORANGE: &str = "\x1b[38;5;208m"; // #FF8700 — accent
pub const BOLD_NEON_ORANGE: &str = "\x1b[1;38;5;208m";
pub const NEON_VIOLET: &str = "\x1b[38;5;141m"; // #AF87FF — divider/decoration
pub const BOLD_NEON_VIOLET: &str = "\x1b[1;38;5;141m";
pub const NEON_BLUE: &str = "\x1b[38;5;39m"; // #00AFFF — links/hints
pub const BOLD_NEON_BLUE: &str = "\x1b[1;38;5;39m";

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
pub const BG_INPUT: &str = "\x1b[48;5;235m"; // Slightly lighter dark bg for input line
pub const CLEAR_LINE: &str = "\x1b[2K"; // Clear entire line

/// Print a subtle horizontal divider — used between conversation turns so
/// the scrollback reads as discrete cards without a heavy frame.
///
/// Style: neon violet gradient dots, width clamped to terminal.
pub fn print_divider() {
    let w = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);
    let inner = w.saturating_sub(4).min(76);
    // Alternating violet / dim rule for a subtle gradient feel
    let rule: String = "─".repeat(inner);
    println!();
    println!("  {NEON_VIOLET}{DIM}{rule}{RESET}");
    println!();
}

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
