use std::io::{self, Write};

use crate::ui::format::*;
use yangzz_core::render::Renderer;

// ────────────────────────────────────────────────────────────────
// Renderer — Claude Code / nocode visual language
//
//   ❯  user input
//   ⎿  assistant text (continuation prefix)
//   ●  tool call in progress / done ✓
//   ✖  error
//   ∴  thinking spinner
// ────────────────────────────────────────────────────────────────

pub(crate) struct ReplRenderer {
    streaming_text: String,
    streaming_lines: usize,
    spinner: Option<indicatif::ProgressBar>,
    first_token: bool,
    line_count: usize, // lines printed since first_token
}

impl ReplRenderer {
    pub(crate) fn new() -> Self {
        Self {
            streaming_text: String::new(),
            streaming_lines: 0,
            spinner: None,
            first_token: true,
            line_count: 0,
        }
    }

    fn start_spinner(&mut self, msg: &str) {
        let pb = indicatif::ProgressBar::new_spinner();
        pb.set_style(
            indicatif::ProgressStyle::default_spinner()
                .tick_strings(&["∴ ", "∵ ", "∴ ", "∵ ", "∴ ", "∵ "])
                .template("  {spinner:.yellow} {msg}")
                .unwrap(),
        );
        pb.set_message(msg.to_string());
        pb.enable_steady_tick(std::time::Duration::from_millis(120));
        self.spinner = Some(pb);
    }

    pub(crate) fn stop_spinner(&mut self) {
        if let Some(pb) = self.spinner.take() {
            pb.finish_and_clear();
        }
    }

    fn count_display_lines(text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        let tw = crossterm::terminal::size()
            .map(|(w, _)| w as usize)
            .unwrap_or(80);
        text.split('\n')
            .map(|l| {
                let display_width: usize = l
                    .chars()
                    .map(|c| {
                        if ('\u{2E80}'..='\u{9FFF}').contains(&c)
                            || ('\u{F900}'..='\u{FAFF}').contains(&c)
                            || ('\u{FE30}'..='\u{FE4F}').contains(&c)
                            || ('\u{FF01}'..='\u{FF60}').contains(&c)
                            || ('\u{20000}'..='\u{2FA1F}').contains(&c)
                        {
                            2
                        } else {
                            1
                        }
                    })
                    .sum();
                std::cmp::max(1, (display_width + tw - 1) / tw)
            })
            .sum()
    }

    fn has_markdown(text: &str) -> bool {
        text.contains("```") || text.contains("**") || text.contains("##") || text.contains('`')
    }

    fn flush_markdown(&mut self) {
        if self.streaming_text.is_empty() {
            return;
        }
        let raw = std::mem::take(&mut self.streaming_text);
        let text = pangu_skip_code(&raw);

        let term_height = crossterm::terminal::size()
            .map(|(_, h)| h as usize)
            .unwrap_or(24);
        let max_erase = term_height.saturating_sub(2);
        let counter_mode = self.streaming_lines > max_erase;
        let has_md = Self::has_markdown(&text);

        if counter_mode {
            print!("\r\x1b[2K");
            for _ in 0..max_erase.min(self.streaming_lines) {
                print!("\x1b[A\x1b[2K");
            }
            let _ = io::stdout().flush();
            self.render_final(&text, has_md);
        } else if has_md {
            print!("\r");
            for _ in 0..self.streaming_lines {
                print!("\x1b[A\x1b[2K");
            }
            print!("\x1b[2K");
            let _ = io::stdout().flush();
            self.render_final(&text, true);
        } else if !raw.ends_with('\n') {
            println!();
        }

        self.streaming_lines = 0;
        self.first_token = true;
        self.line_count = 0;
    }

    fn render_final(&self, text: &str, use_markdown: bool) {
        let rendered = if use_markdown {
            let skin = make_skin();
            format!("{}", skin.term_text(text))
        } else {
            text.to_string()
        };
        for (i, line) in rendered.lines().enumerate() {
            if i == 0 {
                println!("{GOLD}⎿{RESET} {line}");
            } else {
                println!("  {line}");
            }
        }
        let _ = io::stdout().flush();
    }
}

fn make_skin() -> termimad::MadSkin {
    use termimad::crossterm::style::{Attribute, Color};

    let mut skin = termimad::MadSkin::default();
    skin.code_block.set_fg(Color::AnsiValue(179));
    skin.inline_code.set_fg(Color::AnsiValue(178));
    skin.bold.add_attr(Attribute::Bold);
    skin.italic.set_fg(Color::AnsiValue(179));
    skin.headers[0].set_fg(Color::AnsiValue(178));
    skin.headers[0].add_attr(Attribute::Bold);
    skin.headers[1].set_fg(Color::AnsiValue(178));
    skin.headers[2].set_fg(Color::AnsiValue(179));
    skin
}

impl Renderer for ReplRenderer {
    fn render_text_delta(&mut self, text: &str) {
        if self.first_token {
            print!("{GOLD}⎿{RESET} ");
            let _ = io::stdout().flush();
            self.first_token = false;
            self.line_count = 0;
        }
        self.streaming_text.push_str(text);
        self.streaming_lines = Self::count_display_lines(&self.streaming_text);

        let term_height = crossterm::terminal::size()
            .map(|(_, h)| h as usize)
            .unwrap_or(24);
        let max_erasable = term_height.saturating_sub(2);

        if self.streaming_lines <= max_erasable {
            print!("{text}");
            let _ = io::stdout().flush();
        } else {
            print!(
                "\r\x1b[2K  {DIM}[streaming... {} chars]{RESET}",
                self.streaming_text.len()
            );
            let _ = io::stdout().flush();
        }
    }

    fn render_tool_start(&mut self, name: &str, _id: &str) {
        self.flush_markdown();
        println!("  {SOFT_GOLD}●{RESET} {BOLD}{name}{RESET} {DIM}…{RESET}");
    }

    fn render_tool_result(&mut self, name: &str, result: &str, is_error: bool) {
        if is_error {
            println!("  {RED}✖ {BOLD}{name}{RESET}");
            for line in result.lines().take(5) {
                println!("  {DIM}⎿{RESET} {RED}{line}{RESET}");
            }
        } else {
            let first_line = result.lines().next().unwrap_or("");
            let preview: String = first_line.chars().take(80).collect();
            println!("  {GOLD}●{RESET} {BOLD}{name}{RESET} {GREEN}✓{RESET}");
            if !preview.is_empty() {
                println!("  {DIM}⎿{RESET} {DIM}{preview}{RESET}");
            }
        }
    }

    fn render_error(&mut self, message: &str) {
        self.stop_spinner();
        eprintln!("  {RED}✖{RESET} {RED}{message}{RESET}");
    }

    fn render_info(&mut self, message: &str) {
        println!("  {DIM}•{RESET} {SOFT_GOLD}{message}{RESET}");
    }

    fn render_complete(&mut self) {
        self.flush_markdown();
        println!();
    }

    fn render_status(&mut self, _status: &str) {}

    fn render_thinking_start(&mut self) {
        self.start_spinner("Thinking...");
    }

    fn render_thinking_stop(&mut self) {
        self.stop_spinner();
    }
}

fn pangu_skip_code(text: &str) -> String {
    let mut result = String::new();
    let mut in_code_block = false;

    for line in text.split('\n') {
        if !result.is_empty() {
            result.push('\n');
        }
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            result.push_str(line);
        } else if in_code_block {
            result.push_str(line);
        } else {
            result.push_str(&yangzz_core::pangu::spacing(line));
        }
    }
    result
}
