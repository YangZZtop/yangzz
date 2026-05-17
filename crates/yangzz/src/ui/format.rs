// ── ANSI escape codes — adaptive palette ──
//
// Detects terminal background (light vs dark) and uses appropriate colors.
// On light themes, uses darker saturated colors visible on white backgrounds.
// On dark themes, uses bright neon colors that pop.
//
// User override: set YANGZZ_THEME=light or YANGZZ_THEME=dark

use std::fmt;
use std::sync::OnceLock;

/// Terminal theme
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermTheme {
    Dark,
    Light,
}

/// Detect terminal theme
fn detect_theme() -> TermTheme {
    // 1. Explicit user override
    if let Ok(val) = std::env::var("YANGZZ_THEME") {
        return match val.to_lowercase().as_str() {
            "light" | "白" | "l" => TermTheme::Light,
            _ => TermTheme::Dark,
        };
    }

    // 2. COLORFGBG (set by rxvt, xterm, some terminals)
    if let Ok(val) = std::env::var("COLORFGBG") {
        if let Some(bg) = val.split(';').last() {
            if let Ok(n) = bg.parse::<u32>() {
                return if n >= 7 && n != 8 {
                    TermTheme::Light
                } else {
                    TermTheme::Dark
                };
            }
        }
    }

    // 3. macOS Terminal.app defaults to light profile
    if let Ok(term) = std::env::var("TERM_PROGRAM") {
        if term == "Apple_Terminal" {
            return TermTheme::Light;
        }
    }

    TermTheme::Dark
}

/// Cached theme
pub fn theme() -> TermTheme {
    static THEME: OnceLock<TermTheme> = OnceLock::new();
    *THEME.get_or_init(detect_theme)
}

pub fn is_light_theme() -> bool {
    theme() == TermTheme::Light
}

// ── Adaptive color type ──
// Implements Display so it can be used directly in format!() strings.

pub struct Color {
    dark: &'static str,
    light: &'static str,
}

impl Copy for Color {}
impl Clone for Color {
    fn clone(&self) -> Self { *self }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if is_light_theme() {
            f.write_str(self.light)
        } else {
            f.write_str(self.dark)
        }
    }
}

// ── Core escapes (theme-independent) ──
pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const ITALIC: &str = "\x1b[3m";
pub const CLEAR_LINE: &str = "\x1b[2K";

// ── Adaptive colors ──
// Used in format!() like: println!("  {DIM}text{RESET}")

pub static DIM: Color = Color { dark: "\x1b[2m", light: "\x1b[38;5;242m" };
pub static GOLD: Color = Color { dark: "\x1b[38;5;178m", light: "\x1b[38;5;130m" };
pub static BOLD_GOLD: Color = Color { dark: "\x1b[1;38;5;178m", light: "\x1b[1;38;5;130m" };
pub static SOFT_GOLD: Color = Color { dark: "\x1b[38;5;179m", light: "\x1b[38;5;136m" };

pub static GREEN: Color = Color { dark: "\x1b[32m", light: "\x1b[38;5;28m" };
pub static RED: Color = Color { dark: "\x1b[31m", light: "\x1b[38;5;160m" };
pub static YELLOW: Color = Color { dark: "\x1b[33m", light: "\x1b[38;5;130m" };
pub static MAGENTA: Color = Color { dark: "\x1b[35m", light: "\x1b[38;5;127m" };
pub static BLUE: Color = Color { dark: "\x1b[34m", light: "\x1b[38;5;25m" };
pub static CYAN: Color = Color { dark: "\x1b[36m", light: "\x1b[38;5;30m" };
pub static WHITE: Color = Color { dark: "\x1b[37m", light: "\x1b[38;5;235m" };

pub static BOLD_GREEN: Color = Color { dark: "\x1b[1;32m", light: "\x1b[1;38;5;28m" };
pub static BOLD_RED: Color = Color { dark: "\x1b[1;31m", light: "\x1b[1;38;5;160m" };
pub static BOLD_YELLOW: Color = Color { dark: "\x1b[1;33m", light: "\x1b[1;38;5;130m" };
pub static BOLD_MAGENTA: Color = Color { dark: "\x1b[1;35m", light: "\x1b[1;38;5;127m" };
pub static BOLD_BLUE: Color = Color { dark: "\x1b[1;34m", light: "\x1b[1;38;5;25m" };
pub static BOLD_CYAN: Color = Color { dark: "\x1b[1;36m", light: "\x1b[1;38;5;30m" };

// Neon palette (status bar / accents)
pub static NEON_PINK: Color = Color { dark: "\x1b[38;5;201m", light: "\x1b[38;5;162m" };
pub static BOLD_NEON_PINK: Color = Color { dark: "\x1b[1;38;5;201m", light: "\x1b[1;38;5;162m" };
pub static NEON_CYAN: Color = Color { dark: "\x1b[38;5;51m", light: "\x1b[38;5;30m" };
pub static BOLD_NEON_CYAN: Color = Color { dark: "\x1b[1;38;5;51m", light: "\x1b[1;38;5;30m" };
pub static NEON_GREEN: Color = Color { dark: "\x1b[38;5;46m", light: "\x1b[38;5;28m" };
pub static BOLD_NEON_GREEN: Color = Color { dark: "\x1b[1;38;5;46m", light: "\x1b[1;38;5;28m" };
pub static NEON_YELLOW: Color = Color { dark: "\x1b[38;5;226m", light: "\x1b[38;5;136m" };
pub static BOLD_NEON_YELLOW: Color = Color { dark: "\x1b[1;38;5;226m", light: "\x1b[1;38;5;136m" };
pub static NEON_ORANGE: Color = Color { dark: "\x1b[38;5;208m", light: "\x1b[38;5;166m" };
pub static BOLD_NEON_ORANGE: Color = Color { dark: "\x1b[1;38;5;208m", light: "\x1b[1;38;5;166m" };
pub static NEON_VIOLET: Color = Color { dark: "\x1b[38;5;141m", light: "\x1b[38;5;91m" };
pub static BOLD_NEON_VIOLET: Color = Color { dark: "\x1b[1;38;5;141m", light: "\x1b[1;38;5;91m" };
pub static NEON_BLUE: Color = Color { dark: "\x1b[38;5;39m", light: "\x1b[38;5;25m" };
pub static BOLD_NEON_BLUE: Color = Color { dark: "\x1b[1;38;5;39m", light: "\x1b[1;38;5;25m" };

// Backgrounds
pub static BG_DARK: Color = Color { dark: "\x1b[48;5;236m", light: "\x1b[48;5;254m" };
pub static BG_INPUT: Color = Color { dark: "\x1b[48;5;235m", light: "\x1b[48;5;255m" };

/// Print a subtle horizontal divider
pub fn print_divider() {
    let w = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);
    let inner = w.saturating_sub(4).min(76);
    let rule: String = "─".repeat(inner);
    println!();
    println!("  {NEON_VIOLET}{DIM}{rule}{RESET}");
    println!();
}

// ── Utility functions ──

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
    let cost = (input_tokens as f64 * 3.0 + output_tokens as f64 * 15.0) / 1_000_000.0;
    if cost < 0.01 {
        "<$0.01".to_string()
    } else {
        format!("${:.2}", cost)
    }
}
