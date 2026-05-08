use super::format::*;
use super::i18n::t;
use std::process::Command;
use yangzz_core::config::model_meta;

/// Print welcome banner: logo + flat info layout (no box frame)
pub fn print_welcome(model: &str, provider: &str, version: &str) {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let short_cwd = shorten_path(&cwd);
    let git_branch = get_git_branch().unwrap_or_default();
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default();

    let s = t();
    let meta = model_meta::lookup_model(model);

    println!();

    // ── Logo ──
    let logo = [
        r" ██    ██  █████  ███    ██  ██████  ██████ ██████",
        r"  ██  ██  ██   ██ ████   ██ ██         ███    ███ ",
        r"   ████   ███████ ██ ██  ██ ██  ███   ███    ███  ",
        r"    ██    ██   ██ ██  ████  ██   ██  ███    ███   ",
        r"    ██    ██   ██ ██   ███   ██████ ██████ ██████ ",
    ];
    for line in &logo {
        println!("  {BOLD_GOLD}{line}{RESET}");
    }
    println!();

    // ── Brand line ──
    println!(
        "  {BOLD_GOLD}yangzz{RESET} {DIM}v{version}{RESET}  {DIM}·{RESET}  {DIM}{}{RESET}",
        s.tagline
    );
    println!();

    // ── Model line ──
    println!("  {BOLD_GOLD}{model}{RESET}  {DIM}via{RESET} {provider}");

    if let Some(m) = meta {
        let ctx_str = model_meta::format_context(m.context_window);
        let price_in = model_meta::format_price(m.input_price);
        let price_out = model_meta::format_price(m.output_price);

        let mut parts = vec![
            format!("{DIM}{ctx_str} ctx{RESET}"),
            format!("{GREEN}{price_in}{RESET}{DIM}/M in{RESET}"),
            format!("{GOLD}{price_out}{RESET}{DIM}/M out{RESET}"),
        ];
        if let Some(cr) = m.cache_read_price {
            parts.push(format!(
                "{GREEN}{}{RESET}{DIM}/M cache{RESET}",
                model_meta::format_price(cr)
            ));
        }
        if m.supports_reasoning {
            let effort = m.reasoning_effort.unwrap_or("medium");
            parts.push(format!(
                "{MAGENTA}◆{RESET} {DIM}reasoning{RESET} {MAGENTA}{effort}{RESET}"
            ));
        }
        println!("  {}", parts.join(&format!("  {DIM}·{RESET}  ")));
    }
    println!();

    // ── Workspace line ──
    let mut ws_line = format!("  {SOFT_GOLD}{short_cwd}{RESET}");
    if !git_branch.is_empty() {
        ws_line.push_str(&format!("  {DIM}·{RESET}  {GREEN}{git_branch}{RESET}"));
    }
    println!("{ws_line}");
    println!();

    // ── Greeting + hints ──
    if user.is_empty() {
        println!("  {BOLD}{}{RESET}", s.welcome_back);
    } else {
        println!("  {}{BOLD_GOLD}{user}{RESET}！", s.welcome_back_user);
    }
    println!("  {DIM}{}{RESET}", s.banner_hint);
    println!();
}

pub fn shorten_path(path: &str) -> String {
    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();
    if !home.is_empty() && path.starts_with(&home) {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    }
}

pub fn get_git_branch() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}
