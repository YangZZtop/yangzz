use std::io::{self, Write};
use std::time::Instant;

use crate::ui::format::*;
use yangzz_core::render::Renderer;

pub(crate) struct ReplRenderer {
    streaming_text: String,
    streaming_lines: usize,
    spinner: Option<indicatif::ProgressBar>,
    first_token: bool,
    line_count: usize,
    tool_start_time: Option<Instant>,
    current_tool: Option<String>,
}

impl ReplRenderer {
    pub(crate) fn new() -> Self {
        Self {
            streaming_text: String::new(),
            streaming_lines: 0,
            spinner: None,
            first_token: true,
            line_count: 0,
            tool_start_time: None,
            current_tool: None,
        }
    }

    fn start_spinner(&mut self, msg: &str) {
        let pb = indicatif::ProgressBar::new_spinner();
        pb.set_style(
            indicatif::ProgressStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                .template("  {spinner:.cyan} {msg}")
                .unwrap(),
        );
        pb.set_message(msg.to_string());
        pb.enable_steady_tick(std::time::Duration::from_millis(80));
        self.spinner = Some(pb);
    }

    fn update_spinner(&mut self, msg: &str) {
        if let Some(pb) = &self.spinner {
            pb.set_message(msg.to_string());
        } else {
            self.start_spinner(msg);
        }
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
        for line in rendered.lines() {
            println!("  {line}");
        }
        let _ = io::stdout().flush();
    }

    fn tool_icon(name: &str) -> &'static str {
        match name {
            "bash" => "⚡",
            "file_read" | "notebook_read" => "📖",
            "file_write" | "file_append" => "✏️",
            "file_edit" | "multi_edit" | "parallel_edit" => "🔧",
            "grep" => "🔍",
            "glob" | "list_dir" | "tree" => "📂",
            "fetch" => "🌐",
            "sub_agent" => "🤖",
            "ask_user" => "💬",
            _ => "●",
        }
    }

    fn format_elapsed(start: Instant) -> String {
        let elapsed = start.elapsed();
        if elapsed.as_millis() < 1000 {
            format!("{}ms", elapsed.as_millis())
        } else {
            format!("{:.1}s", elapsed.as_secs_f64())
        }
    }

    fn render_tool_output_lines(result: &str, max_lines: usize) {
        let lines: Vec<&str> = result.lines().collect();
        let total = lines.len();
        if total <= max_lines {
            for line in &lines {
                let trimmed: String = line.chars().take(120).collect();
                println!("    {DIM}{trimmed}{RESET}");
            }
        } else {
            let head = max_lines / 2;
            let tail = max_lines - head - 1;
            for line in &lines[..head] {
                let trimmed: String = line.chars().take(120).collect();
                println!("    {DIM}{trimmed}{RESET}");
            }
            println!("    {DIM}… ({} lines hidden){RESET}", total - head - tail);
            for line in &lines[total - tail..] {
                let trimmed: String = line.chars().take(120).collect();
                println!("    {DIM}{trimmed}{RESET}");
            }
        }
    }
}

fn make_skin() -> termimad::MadSkin {
    use termimad::crossterm::style::{Attribute, Color};

    let mut skin = termimad::MadSkin::default();
    skin.code_block.set_fg(Color::AnsiValue(252));
    skin.code_block.set_bg(Color::AnsiValue(235));
    skin.inline_code.set_fg(Color::AnsiValue(178));
    skin.inline_code.set_bg(Color::AnsiValue(236));
    skin.bold.add_attr(Attribute::Bold);
    skin.bold.set_fg(Color::White);
    skin.italic.add_attr(Attribute::Italic);
    skin.italic.set_fg(Color::AnsiValue(179));
    skin.headers[0].set_fg(Color::AnsiValue(51));
    skin.headers[0].add_attr(Attribute::Bold);
    skin.headers[1].set_fg(Color::AnsiValue(45));
    skin.headers[1].add_attr(Attribute::Bold);
    skin.headers[2].set_fg(Color::AnsiValue(39));
    skin.bullet.set_fg(Color::AnsiValue(178));
    skin.quote_mark.set_fg(Color::AnsiValue(242));
    skin
}

impl Renderer for ReplRenderer {
    fn render_text_delta(&mut self, text: &str) {
        if self.first_token {
            self.stop_spinner();
            print!("\r\x1b[2K");
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
                "\r\x1b[2K  {DIM}[streaming… {} lines, {} chars]{RESET}",
                self.streaming_text.lines().count(),
                self.streaming_text.len()
            );
            let _ = io::stdout().flush();
        }
    }

    fn render_tool_start(&mut self, name: &str, _id: &str) {
        self.flush_markdown();
        self.tool_start_time = Some(Instant::now());
        self.current_tool = Some(name.to_string());
        let icon = Self::tool_icon(name);
        self.update_spinner(&format!("{icon} {name}"));
    }

    fn render_tool_start_with_input(&mut self, name: &str, id: &str, input: &serde_json::Value) {
        let _ = id;
        self.flush_markdown();
        self.tool_start_time = Some(Instant::now());
        self.current_tool = Some(name.to_string());
        let icon = Self::tool_icon(name);
        let context = tool_context_msg(name, input);
        self.update_spinner(&format!("{icon} {context}"));
    }

    fn render_tool_result(&mut self, name: &str, result: &str, is_error: bool) {
        self.stop_spinner();
        let icon = Self::tool_icon(name);
        let duration = self
            .tool_start_time
            .map(Self::format_elapsed)
            .unwrap_or_default();
        self.tool_start_time = None;
        self.current_tool = None;

        if is_error {
            println!("  {icon} {BOLD}{name}{RESET} {RED}✖{RESET} {DIM}{duration}{RESET}");
            let lines: Vec<&str> = result.lines().take(10).collect();
            for line in lines {
                let trimmed: String = line.chars().take(120).collect();
                println!("    {RED}{trimmed}{RESET}");
            }
            if result.lines().count() > 10 {
                let total = result.lines().count();
                println!("    {DIM}… ({} more lines){RESET}", total - 10);
            }
        } else {
            println!("  {icon} {BOLD}{name}{RESET} {GREEN}✓{RESET} {DIM}{duration}{RESET}");
            if !result.is_empty() {
                if is_diff_output(name, result) {
                    render_diff_lines(result, 12);
                } else {
                    Self::render_tool_output_lines(result, 8);
                }
            }
        }
    }

    fn render_error(&mut self, message: &str) {
        self.stop_spinner();
        eprintln!();
        eprintln!("  {RED}✖{RESET} {BOLD_RED}{message}{RESET}");
        if let Some(hint) = error_hint(message) {
            eprintln!("  {DIM}  → {hint}{RESET}");
        }
    }

    fn render_info(&mut self, message: &str) {
        self.stop_spinner();
        println!("  {DIM}•{RESET} {SOFT_GOLD}{message}{RESET}");
    }

    fn render_complete(&mut self) {
        self.flush_markdown();
        println!();
    }

    fn render_thinking_delta(&mut self, text: &str) {
        if self.first_token {
            self.stop_spinner();
            print!("\r\x1b[2K");
            println!("  {DIM}💭 Thinking...{RESET}");
            self.first_token = false;
        }
        print!("{DIM}{text}{RESET}");
        let _ = io::stdout().flush();
    }

    fn render_status(&mut self, status: &str) {
        println!("  {DIM}{status}{RESET}");
    }

    fn render_thinking_start(&mut self) {
        self.start_spinner("Thinking…");
    }

    fn render_thinking_stop(&mut self) {
        self.stop_spinner();
    }
}

fn is_diff_output(tool_name: &str, result: &str) -> bool {
    if matches!(tool_name, "file_edit" | "multi_edit" | "parallel_edit") {
        return true;
    }
    let first_lines: Vec<&str> = result.lines().take(5).collect();
    first_lines.iter().any(|l| l.starts_with("---") || l.starts_with("+++") || l.starts_with("@@"))
}

fn render_diff_lines(result: &str, max_lines: usize) {
    let lines: Vec<&str> = result.lines().collect();
    let total = lines.len();
    let show = total.min(max_lines);
    for line in &lines[..show] {
        let trimmed: String = line.chars().take(120).collect();
        if line.starts_with('+') && !line.starts_with("+++") {
            println!("    {GREEN}{trimmed}{RESET}");
        } else if line.starts_with('-') && !line.starts_with("---") {
            println!("    {RED}{trimmed}{RESET}");
        } else if line.starts_with("@@") {
            println!("    {CYAN}{trimmed}{RESET}");
        } else {
            println!("    {DIM}{trimmed}{RESET}");
        }
    }
    if total > max_lines {
        println!("    {DIM}… ({} more lines){RESET}", total - max_lines);
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

#[allow(dead_code)]
fn short_path(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn tool_context_msg(name: &str, input: &serde_json::Value) -> String {
    match name {
        "bash" => {
            if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                let short: String = cmd.chars().take(60).collect();
                if cmd.len() > 60 {
                    format!("bash: {short}…")
                } else {
                    format!("bash: {short}")
                }
            } else {
                "bash".into()
            }
        }
        "file_read" | "notebook_read" => {
            if let Some(p) = input.get("path").and_then(|v| v.as_str()) {
                format!("Reading {}", short_path(p))
            } else {
                "Reading file…".into()
            }
        }
        "file_write" | "file_append" => {
            if let Some(p) = input.get("path").and_then(|v| v.as_str()) {
                format!("Writing {}", short_path(p))
            } else {
                "Writing…".into()
            }
        }
        "file_edit" | "multi_edit" | "parallel_edit" => {
            if let Some(p) = input.get("path").and_then(|v| v.as_str()) {
                format!("Editing {}", short_path(p))
            } else {
                "Editing…".into()
            }
        }
        "grep" => {
            if let Some(pat) = input.get("pattern").and_then(|v| v.as_str()) {
                let short: String = pat.chars().take(30).collect();
                format!("Searching: {short}")
            } else {
                "Searching…".into()
            }
        }
        "glob" => {
            if let Some(pat) = input.get("pattern").and_then(|v| v.as_str()) {
                format!("Finding: {pat}")
            } else {
                "Finding files…".into()
            }
        }
        "fetch" => {
            if let Some(url) = input.get("url").and_then(|v| v.as_str()) {
                let short: String = url.chars().take(50).collect();
                format!("Fetching {short}")
            } else {
                "Fetching…".into()
            }
        }
        "list_dir" | "tree" => {
            if let Some(p) = input.get("path").and_then(|v| v.as_str()) {
                format!("Listing {}", short_path(p))
            } else {
                "Listing directory…".into()
            }
        }
        _ => name.to_string(),
    }
}

fn error_hint(message: &str) -> Option<&'static str> {
    let lower = message.to_lowercase();
    if lower.contains("authentication") || lower.contains("401") || lower.contains("invalid api key") {
        Some("Check your API key with /key or set ANTHROPIC_API_KEY")
    } else if lower.contains("rate limit") || lower.contains("429") {
        Some("Rate limited — wait a moment and try again, or switch model with /model")
    } else if lower.contains("timeout") || lower.contains("timed out") {
        Some("Request timed out — check your network or try a shorter prompt")
    } else if lower.contains("connection") || lower.contains("network") || lower.contains("dns") {
        Some("Network error — check your internet connection")
    } else if lower.contains("context") && lower.contains("too long") {
        Some("Context too large — use /compact or start a /new conversation")
    } else if lower.contains("overloaded") || lower.contains("503") || lower.contains("529") {
        Some("API overloaded — retry in a few seconds or switch provider with /model")
    } else if lower.contains("cancelled") {
        None
    } else {
        None
    }
}
