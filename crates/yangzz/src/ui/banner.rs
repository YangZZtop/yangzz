use super::format::*;
use super::i18n::t;
use std::process::Command;
use yangzz_core::config::model_meta;

/// Box-drawing helpers
fn box_top(w: usize) -> String {
    format!("  {DIM}╭{}╮{RESET}", "─".repeat(w))
}
fn box_mid(w: usize) -> String {
    format!("  {DIM}├{}┤{RESET}", "─".repeat(w))
}
fn box_bot(w: usize) -> String {
    format!("  {DIM}╰{}╯{RESET}", "─".repeat(w))
}
fn box_line(w: usize, content: &str) -> String {
    // strip ANSI for length calculation
    let visible_len = strip_ansi_len(content);
    let pad = if w > visible_len + 2 {
        w - visible_len - 2
    } else {
        0
    };
    format!("  {DIM}│{RESET} {content}{} {DIM}│{RESET}", " ".repeat(pad))
}

fn strip_ansi_len(s: &str) -> usize {
    let mut len = 0usize;
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
        // CJK characters take 2 columns
        if ('\u{4e00}'..='\u{9fff}').contains(&ch)
            || ('\u{3400}'..='\u{4dbf}').contains(&ch)
            || ('\u{f900}'..='\u{faff}').contains(&ch)
            || ('\u{ff00}'..='\u{ffef}').contains(&ch)
        {
            len += 2;
        } else {
            len += 1;
        }
    }
    len
}

/// Print welcome banner with box-drawing frames
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

    let term_w = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);
    let bw = term_w.min(62).max(40); // box inner width
    let s = t();
    let meta = model_meta::lookup_model(model);

    println!();

    // ═══ ASCII Art Logo (Y A N G Z Z) ═══
    //  Y        A        N        G        Z        Z
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

    // ═══ Header box: brand + version ═══
    println!("{}", box_top(bw));
    println!(
        "{}",
        box_line(
            bw,
            &format!(
                "{BOLD_GOLD}yangzz{RESET}  {DIM}v{version}{RESET}  {DIM}·{RESET}  {DIM}{}{RESET}",
                s.tagline
            )
        )
    );
    println!("{}", box_mid(bw));

    // ═══ Model info block ═══
    {
        let model_line =
            format!("{BOLD_GOLD}{model}{RESET}  {DIM}via{RESET} {DIM}{provider}{RESET}");
        println!("{}", box_line(bw, &model_line));

        if let Some(m) = meta {
            let ctx_str = model_meta::format_context(m.context_window);
            let price_in = model_meta::format_price(m.input_price);
            let price_out = model_meta::format_price(m.output_price);

            let info = format!(
                "{DIM}{ctx_str} context{RESET}  {DIM}·{RESET}  {GREEN}{price_in}{RESET}{DIM}/M in{RESET}  {GOLD}{price_out}{RESET}{DIM}/M out{RESET}"
            );
            println!("{}", box_line(bw, &info));

            // Cache pricing
            if m.cache_read_price.is_some() || m.cache_write_price.is_some() {
                let mut cache_parts = Vec::new();
                if let Some(cr) = m.cache_read_price {
                    cache_parts.push(format!(
                        "{DIM}cache read{RESET} {GREEN}{}{RESET}{DIM}/M{RESET}",
                        model_meta::format_price(cr)
                    ));
                }
                if let Some(cw) = m.cache_write_price {
                    cache_parts.push(format!(
                        "{DIM}cache write{RESET} {SOFT_GOLD}{}{RESET}{DIM}/M{RESET}",
                        model_meta::format_price(cw)
                    ));
                }
                println!(
                    "{}",
                    box_line(bw, &cache_parts.join(&format!("  {DIM}·{RESET}  ")))
                );
            }

            // Reasoning
            if m.supports_reasoning {
                let effort = m.reasoning_effort.unwrap_or("medium");
                println!(
                    "{}",
                    box_line(
                        bw,
                        &format!(
                            "{MAGENTA}◆{RESET} {DIM}reasoning{RESET}  {MAGENTA}{effort}{RESET}"
                        )
                    )
                );
            }
        }
    }

    println!("{}", box_mid(bw));

    // ═══ Workspace block ═══
    {
        let mut ws_line = format!("{SOFT_GOLD}{short_cwd}{RESET}");
        if !git_branch.is_empty() {
            ws_line.push_str(&format!("  {GREEN}{git_branch}{RESET}"));
        }
        println!("{}", box_line(bw, &ws_line));
    }

    println!("{}", box_bot(bw));

    // ═══ Greeting (outside box) ═══
    println!();
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
