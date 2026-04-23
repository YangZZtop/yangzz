use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Instant;
use yangzz_core::config;
use yangzz_core::config::settings::CliOverrides;
use yangzz_core::config::Settings;
use yangzz_core::message::Message;
use yangzz_core::provider::Provider;
use yangzz_core::query;
use yangzz_core::render::Renderer;
use yangzz_core::session::Session;
use yangzz_core::skill::{self, Skill};
use yangzz_core::memory;
use yangzz_core::tool::ToolExecutor;

use crate::ui::format::*;
use crate::ui::i18n::{t, translate_tool_desc, translate_skill_desc};
use crate::ui::{banner, select, status};

const SYSTEM_PROMPT: &str = r#"You are yangzz, an AI coding assistant running in the user's terminal.

You have access to 14 tools: bash, file_read, file_write, file_edit, file_append, multi_edit, grep, glob, list_dir, tree, fetch, ask_user, notebook_read, notebook_edit.

Guidelines:
- Read files before editing to understand context. file_read returns the full file by default.
- Use file_edit for precise single changes (old_string must match exactly and be unique).
- Use multi_edit for multiple changes to the same file in one operation.
- Use file_write for creating new files, file_append for appending.
- Use bash for running commands, tests, installing packages.
- Use grep to search content, glob to find files by pattern, list_dir/tree for directory structure.
- Use fetch to retrieve web content.
- Use ask_user when you need clarification from the user.
- Use notebook_read/notebook_edit for Jupyter notebooks.
- Be concise in explanations. Show your work through tool usage.
- When editing code, preserve existing style and conventions.
- Always respond in the same language as the user's message."#;

// ────────────────────────────────────────────────────────────────
// Renderer — Claude Code / nocode visual language
//
//   ❯  user input
//   ⎿  assistant text (continuation prefix)
//   ●  tool call in progress / done ✓
//   ✖  error
//   ∴  thinking spinner
// ────────────────────────────────────────────────────────────────

struct ReplRenderer {
    streaming_text: String,
    streaming_lines: usize,
    spinner: Option<indicatif::ProgressBar>,
    first_token: bool,
    line_count: usize, // lines printed since first_token
}

impl ReplRenderer {
    fn new() -> Self {
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

    fn stop_spinner(&mut self) {
        if let Some(pb) = self.spinner.take() {
            pb.finish_and_clear();
        }
    }

    fn count_display_lines(text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        let tw = crossterm::terminal::size().map(|(w, _)| w as usize).unwrap_or(80);
        text.split('\n')
            .map(|l| {
                // CJK-aware display width: CJK chars = 2 columns, others = 1
                let display_width: usize = l.chars().map(|c| {
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
                }).sum();
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
        // Apply Pangu CJK spacing (skip code blocks)
        let text = pangu_skip_code(&raw);

        if self.streaming_lines > 0 && Self::has_markdown(&text) {
            // Determine how many lines we can safely erase
            let term_height = crossterm::terminal::size()
                .map(|(_, h)| h as usize)
                .unwrap_or(24);
            let max_erase = term_height.saturating_sub(2);

            if self.streaming_lines <= max_erase {
                // Short enough: erase raw output and re-render
                print!("\r");
                for _ in 0..self.streaming_lines {
                    print!("\x1b[A\x1b[2K");
                }
                print!("\x1b[2K");
                let _ = io::stdout().flush();
            } else {
                // Long output: raw was suppressed (showed progress counter)
                // Erase the progress counter line
                print!("\r\x1b[2K");
                let _ = io::stdout().flush();
            }

            // Render with markdown
            let skin = make_skin();
            let formatted = skin.term_text(&text);
            for (i, line) in format!("{formatted}").lines().enumerate() {
                if i == 0 {
                    println!("{GOLD}⎿{RESET} {line}");
                } else {
                    println!("  {line}");
                }
            }
            let _ = io::stdout().flush();
        }

        self.streaming_lines = 0;
        self.first_token = true;
        self.line_count = 0;
    }
}

fn make_skin() -> termimad::MadSkin {
    use termimad::crossterm::style::{Attribute, Color};
    let mut skin = termimad::MadSkin::default();
    // Warm gold palette for markdown
    skin.code_block.set_fg(Color::AnsiValue(179)); // soft gold
    skin.inline_code.set_fg(Color::AnsiValue(178)); // gold
    skin.bold.add_attr(Attribute::Bold);
    skin.italic.set_fg(Color::AnsiValue(179));
    skin.headers[0].set_fg(Color::AnsiValue(178)); // gold headers
    skin.headers[0].add_attr(Attribute::Bold);
    skin.headers[1].set_fg(Color::AnsiValue(178));
    skin.headers[2].set_fg(Color::AnsiValue(179));
    skin
}

impl Renderer for ReplRenderer {
    fn render_text_delta(&mut self, text: &str) {
        if self.first_token {
            // Claude Code style: ⎿ prefix for first line of assistant response
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
            // Short enough to erase later — stream raw text for responsiveness
            print!("{text}");
            let _ = io::stdout().flush();
        } else if self.streaming_lines == max_erasable + 1 {
            // Just crossed the threshold — show progress indicator instead
            print!("\r\x1b[2K  {DIM}[streaming... {} chars]{RESET}", self.streaming_text.len());
            let _ = io::stdout().flush();
        } else {
            // Still accumulating — update counter
            print!("\r\x1b[2K  {DIM}[streaming... {} chars]{RESET}", self.streaming_text.len());
            let _ = io::stdout().flush();
        }
    }

    fn render_tool_start(&mut self, name: &str, _id: &str) {
        self.flush_markdown();
        // nocode style: ● ToolName …
        println!("  {SOFT_GOLD}●{RESET} {BOLD}{name}{RESET} {DIM}…{RESET}");
    }

    fn render_tool_result(&mut self, name: &str, result: &str, is_error: bool) {
        if is_error {
            // ✖ ToolName
            println!("  {RED}✖ {BOLD}{name}{RESET}");
            // Show error with ⎿ prefix
            for line in result.lines().take(5) {
                println!("  {DIM}⎿{RESET} {RED}{line}{RESET}");
            }
        } else {
            // ● ToolName ✓
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

// ────────────────────────────────────────────────────────────────
// Single-shot mode
// ────────────────────────────────────────────────────────────────

pub async fn single_shot(
    provider: &Arc<dyn Provider>,
    model: &str,
    max_tokens: u32,
    prompt: &str,
    executor: &ToolExecutor,
) -> Result<()> {
    let mut renderer = ReplRenderer::new();
    let mut messages = vec![Message::user(prompt)];

    query::run_agentic_loop(
        provider, model, max_tokens, &mut messages,
        Some(SYSTEM_PROMPT.to_string()), executor, &mut renderer,
    )
    .await?;
    Ok(())
}

// ────────────────────────────────────────────────────────────────
// Interactive REPL
// ────────────────────────────────────────────────────────────────

pub async fn run(
    provider: &Arc<dyn Provider>,
    model: &str,
    max_tokens: u32,
    executor: &ToolExecutor,
    settings: &Settings,
) -> Result<()> {
    let mut renderer = ReplRenderer::new();
    let mut current_model = model.to_string();
    let mut current_provider: Arc<dyn Provider> = Arc::clone(provider);
    let mut stats = status::SessionStats::new(model, provider.name());

    // Session
    let mut session = Session::new(model, provider.name());
    let mut messages: Vec<Message> = Vec::new();

    // Skills
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut skills = skill::builtin_skills();
    skills.extend(skill::load_skills(&cwd));

    // ── Welcome ──
    banner::print_welcome(&current_model, provider.name(), env!("CARGO_PKG_VERSION"));

    let mut rl = DefaultEditor::new().expect("Failed to init readline");
    // Gold chip ❯ + light warm background for input area (was 236, now 238 + white text)
    let prompt = format!("\x1b[48;5;178m\x1b[1;30m ❯ \x1b[0m ");
    let prompt_cont = format!("    ");
    loop {
        // Multi-line input: end line with \ to continue
        let mut full_input = String::new();
        let mut is_continuation = false;
        loop {
            let p = if is_continuation { &prompt_cont } else { &prompt };
            let readline = rl.readline(p);
            match readline {
                Ok(line) => {
                    print!("{RESET}");
                    let _ = io::stdout().flush();
                    if line.trim_end().ends_with('\\') {
                        let trimmed = line.trim_end().strip_suffix('\\').unwrap_or(&line);
                        full_input.push_str(trimmed);
                        full_input.push('\n');
                        is_continuation = true;
                    } else {
                        full_input.push_str(&line);
                        break;
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    if is_continuation {
                        // Cancel multi-line, reset
                        full_input.clear();
                        break;
                    }
                    println!();
                    full_input.clear();
                    break;
                }
                Err(ReadlineError::Eof) => {
                    if is_continuation && !full_input.is_empty() {
                        break; // Submit what we have
                    }
                    return Ok(()); // exit REPL
                }
                Err(_) => return Ok(()),
            }
        }

        let input = full_input.trim();
        if input.is_empty() {
            continue;
        }
        let _ = rl.add_history_entry(input);

        // ── Slash commands ──
        if input.starts_with('/') {
            let parts: Vec<&str> = input.splitn(2, ' ').collect();
            let cmd = parts[0];
            let arg = parts.get(1).map(|s| s.trim()).unwrap_or("");

            // /model is handled here (async) to fetch models from ALL providers
            if cmd == "/model" || cmd == "/m" {
                let msg_count = messages.len();
                if arg.is_empty() {
                    // Fetch models from ALL available providers
                    print!("  {DIM}{}{RESET}", t().fetching_models);
                    let _ = io::stdout().flush();

                    let available = config::list_available_providers(Some(&current_provider), settings);
                    let mut provider_models: Vec<select::ProviderModels> = Vec::new();
                    for ap in &available {
                        let models = ap.provider.list_models().await.unwrap_or_default();
                        provider_models.push(select::ProviderModels {
                            provider_name: ap.name.clone(),
                            models,
                        });
                    }

                    // Clear "Fetching…" line
                    print!("\r\x1b[2K");
                    let _ = io::stdout().flush();

                    if let Some((sel_model, sel_provider)) =
                        select::select_model(&current_model, current_provider.name(), &provider_models)
                    {
                        if !sel_model.is_empty() {
                            switch_model_provider(
                                &sel_model, Some(&sel_provider),
                                &mut current_model, &mut current_provider,
                                &mut stats, &mut renderer,
                            );
                            if msg_count > 0 {
                                println!("  {DIM}{}{RESET}", t().history_kept.replace("{}", &msg_count.to_string()));
                            }
                        }
                    }
                } else {
                    let provider_hint = detect_provider_from_model(arg);
                    switch_model_provider(
                        arg, provider_hint,
                        &mut current_model, &mut current_provider,
                        &mut stats, &mut renderer,
                    );
                    if msg_count > 0 {
                        println!("  {DIM}{}{RESET}", t().history_kept.replace("{}", &msg_count.to_string()));
                    }
                }
                continue;
            }

            let handled = handle_command(
                cmd, arg,
                &mut current_model, &mut current_provider,
                &mut messages, &mut session, &mut stats, &mut renderer,
                executor, &skills,
            );

            match handled {
                CommandResult::Continue => continue,
                CommandResult::Quit => break,
                CommandResult::FallThrough => {} // process as chat (skill command)
                CommandResult::Unknown => {
                    // Show command picker
                    let cmds = all_commands(&skills);
                    if let Some(picked) = select::select_command(&cmds) {
                        println!("  {DIM}{} {picked}{RESET}", t().tip_prefix);
                    }
                    continue;
                }
            }
        }

        // ── Build system prompt ──
        let base_system = memory::inject_memory_prompt(SYSTEM_PROMPT, &cwd);
        let system = if let Some(matched) = skill::match_skill(input, &skills) {
            println!("  {GOLD}● skill:{RESET} {BOLD}{}{RESET}", matched.name);
            format!("{}\n\n--- Active Skill: {} ---\n{}", base_system, matched.name, matched.body)
        } else {
            base_system
        };

        println!();
        messages.push(Message::user(input));

        // ── Run agentic loop ──
        let start = Instant::now();
        let result = query::run_agentic_loop(
            &current_provider, &current_model, max_tokens,
            &mut messages, Some(system), executor, &mut renderer,
        )
        .await;
        let elapsed = start.elapsed().as_secs_f64();

        match result {
            Ok(usage) => {
                stats.add_usage(usage.input_tokens, usage.output_tokens);
                status::render_turn_info(elapsed);
                status::render_status_bar(&stats);
            }
            Err(e) => {
                renderer.stop_spinner();
                let msg = format!("{e}");
                if msg.contains("Cancelled by user") {
                    println!("\n  {DIM}(cancelled){RESET}");
                } else {
                    renderer.render_error(&msg);
                }
                messages.pop();
            }
        }
        println!();
    }

    Ok(())
}

// ────────────────────────────────────────────────────────────────
// Command handling
// ────────────────────────────────────────────────────────────────

enum CommandResult {
    Continue,
    Quit,
    FallThrough,
    Unknown,
}

fn handle_command(
    cmd: &str,
    arg: &str,
    current_model: &mut String,
    current_provider: &mut Arc<dyn Provider>,
    messages: &mut Vec<Message>,
    session: &mut Session,
    stats: &mut status::SessionStats,
    renderer: &mut ReplRenderer,
    executor: &ToolExecutor,
    skills: &[Skill],
) -> CommandResult {
    match cmd {
        "/quit" | "/exit" | "/q" => {
            session.messages = messages.clone();
            let _ = session.save();
            println!("  {DIM}{}{RESET}", t().session_saved);
            CommandResult::Quit
        }
        "/help" | "/h" | "/?" => {
            print_help(skills);
            CommandResult::Continue
        }
        "/clear" | "/c" => {
            messages.clear();
            println!("  {SOFT_GOLD}{}{RESET}", t().conversation_cleared);
            CommandResult::Continue
        }

        // /model is handled in async run() above — not here
        "/model" | "/m" => CommandResult::Continue,

        "/provider" | "/p" => {
            if arg.is_empty() {
                println!("  {DIM}{}{RESET} {BOLD}{}{RESET}", t().current_colon, current_provider.name());
                println!("  {DIM}{}{RESET}", t().usage_provider);
            } else {
                let mut settings = Settings::load(CliOverrides::default());
                settings.provider = Some(arg.to_string());
                settings.model = Some(current_model.clone());
                match config::resolve_provider(&settings) {
                    Ok(new_provider) => {
                        *current_model = settings.resolved_model();
                        *current_provider = new_provider;
                        stats.model = current_model.clone();
                        stats.provider = current_provider.name().to_string();
                        println!("  {GREEN}{}{RESET} {} {BOLD_GREEN}{current_model}{RESET}", t().switched_to, current_provider.name());
                    }
                    Err(e) => renderer.render_error(&format!("Cannot switch: {e}")),
                }
            }
            CommandResult::Continue
        }

        "/tools" | "/t" => {
            println!();
            println!("  {BOLD}{}{RESET}", t().tools_title);
            for td in executor.tool_definitions() {
                let desc = translate_tool_desc(&td.name, &td.description);
                println!("    {BOLD_YELLOW}{:<14}{RESET} {DIM}{desc}{RESET}", td.name);
            }
            println!();
            CommandResult::Continue
        }

        "/skills" | "/s" => {
            println!();
            println!("  {BOLD}{}{RESET}", t().skills_title);
            for sk in skills {
                let trigger = sk.triggers.iter().find(|t| t.starts_with('/')).cloned().unwrap_or_default();
                let desc = translate_skill_desc(&sk.name, &sk.description);
                println!("    {GOLD}{:<14}{RESET} {DIM}{desc}{RESET}", trigger);
            }
            println!();
            CommandResult::Continue
        }

        "/status" => {
            status::render_status_bar(stats);
            CommandResult::Continue
        }

        "/undo" | "/u" => {
            let rt = tokio::runtime::Handle::current();
            let msg = rt.block_on(executor.undo()).unwrap_or_else(|| "Nothing to undo".into());
            println!("  {GREEN}↩{RESET} {msg}");
            CommandResult::Continue
        }

        "/compact" => {
            let before = messages.len();
            yangzz_core::query::compact_messages_public(messages);
            let after = messages.len();
            println!("  {DIM}Compacted: {before} → {after} messages{RESET}");
            CommandResult::Continue
        }

        "/memory" => {
            let cwd = std::env::current_dir().unwrap_or_default();
            if arg.is_empty() {
                match yangzz_core::memory::load_memory(&cwd) {
                    Some(mem) => println!("  {DIM}{mem}{RESET}"),
                    None => println!("  {DIM}No MEMORY.md found{RESET}"),
                }
            } else {
                match yangzz_core::memory::append_memory(&cwd, arg) {
                    Ok(()) => println!("  {GREEN}✓{RESET} Saved to MEMORY.md"),
                    Err(e) => println!("  {RED}✖{RESET} {e}"),
                }
            }
            CommandResult::Continue
        }

        "/task" | "/tasks" => {
            // Inline task queue display (read-only for REPL)
            use yangzz_core::task_queue::{TaskQueue, TaskType, TaskPriority};
            static TASK_QUEUE: std::sync::OnceLock<TaskQueue> = std::sync::OnceLock::new();
            let queue = TASK_QUEUE.get_or_init(TaskQueue::new);

            if arg.is_empty() || arg == "list" {
                let list = tokio::runtime::Handle::current().block_on(queue.format_list());
                println!("  {BOLD}Task Queue:{RESET}");
                println!("{list}");
            } else if arg.starts_with("add ") {
                let desc = arg.strip_prefix("add ").unwrap_or("").trim();
                if desc.is_empty() {
                    println!("  {DIM}Usage: /task add <description>{RESET}");
                } else {
                    let id = tokio::runtime::Handle::current()
                        .block_on(queue.enqueue(TaskType::Agent, desc, TaskPriority::Normal));
                    println!("  {GREEN}✓{RESET} Task #{id} queued: {desc}");
                }
            } else if arg.starts_with("done ") {
                if let Ok(id) = arg.strip_prefix("done ").unwrap_or("0").trim().parse::<usize>() {
                    tokio::runtime::Handle::current()
                        .block_on(queue.complete(id, "Completed via /task done".into()));
                    println!("  {GREEN}✓{RESET} Task #{id} marked complete");
                }
            } else if arg.starts_with("cancel ") {
                if let Ok(id) = arg.strip_prefix("cancel ").unwrap_or("0").trim().parse::<usize>() {
                    if tokio::runtime::Handle::current().block_on(queue.cancel(id)) {
                        println!("  {GREEN}✓{RESET} Task #{id} cancelled");
                    } else {
                        println!("  {RED}✖{RESET} Cannot cancel task #{id}");
                    }
                }
            } else {
                println!("  {DIM}Usage: /task [list|add <desc>|done <id>|cancel <id>]{RESET}");
            }
            CommandResult::Continue
        }

        "/route" => {
            if arg.is_empty() {
                println!("  {DIM}Usage: /route <prompt> — show which model would be selected{RESET}");
            } else {
                let router = yangzz_core::provider::router::ModelRouter::new();
                let decision = router.route(arg, &["openai", "anthropic", "deepseek", "gemini"]);
                println!("  {BOLD}Complexity:{RESET} {:?}", decision.complexity);
                println!("  {BOLD}Model:{RESET} {}", decision.model);
                println!("  {DIM}{}{RESET}", decision.reason);
            }
            CommandResult::Continue
        }

        "/profile" => {
            let cwd = std::env::current_dir().unwrap_or_default();
            let profile = yangzz_core::skill_detect::detect_project(&cwd);
            println!("  {BOLD}Project Profile:{RESET}");
            if !profile.languages.is_empty() {
                println!("  {GOLD}Languages:{RESET} {}", profile.languages.join(", "));
            }
            if !profile.frameworks.is_empty() {
                println!("  {GOLD}Frameworks:{RESET} {}", profile.frameworks.join(", "));
            }
            if !profile.package_managers.is_empty() {
                println!("  {GOLD}Package Managers:{RESET} {}", profile.package_managers.join(", "));
            }
            if let Some(ref pt) = profile.project_type {
                println!("  {GOLD}Type:{RESET} {pt}");
            }
            println!("  {DIM}Tests: {} | CI: {} | Docker: {} | Git: {}{RESET}",
                if profile.has_tests { "✅" } else { "❌" },
                if profile.has_ci { "✅" } else { "❌" },
                if profile.has_docker { "✅" } else { "❌" },
                if profile.has_git { "✅" } else { "❌" },
            );
            CommandResult::Continue
        }

        "/policy" => {
            let cwd = std::env::current_dir().unwrap_or_default();
            let policy = yangzz_core::sandbox::load_policy(&cwd);
            println!("  {BOLD}Execution Policy:{RESET}");
            println!("  Sandbox: {}", policy.sandbox.mode);
            println!("  Network outbound: {}", if policy.network.allow_outbound { "✅" } else { "❌" });
            if !policy.network.blocked_hosts.is_empty() {
                println!("  Blocked hosts: {}", policy.network.blocked_hosts.join(", "));
            }
            if !policy.commands.blocked_commands.is_empty() {
                println!("  Blocked commands: {}", policy.commands.blocked_commands.join(", "));
            }
            println!("  Max runtime: {}s", policy.commands.max_runtime_secs);
            println!("  Sudo: {}", if policy.commands.allow_sudo { "✅" } else { "❌" });
            CommandResult::Continue
        }

        "/guide" => {
            crate::print_guide();
            CommandResult::Continue
        }

        "/migrate" => {
            let sources = yangzz_core::migrate::detect_sources();
            if sources.is_empty() {
                println!("  {DIM}No migration sources found (Claude Code, Codex CLI, Cursor){RESET}");
            } else {
                for src in &sources {
                    println!("  {GOLD}●{RESET} Found: {} ({} items)", src.tool.name(), src.items.len());
                }
                let cwd = std::env::current_dir().unwrap_or_default();
                match yangzz_core::migrate::migrate_to_memory(&sources, &cwd) {
                    Ok(results) => {
                        for r in &results {
                            println!("  {r}");
                        }
                    }
                    Err(e) => println!("  {RED}✖{RESET} Migration error: {e}"),
                }
            }
            CommandResult::Continue
        }

        "/recall" | "/search" => {
            if arg.is_empty() {
                println!("  {DIM}Usage: /recall <keyword>{RESET}");
            } else {
                let results = yangzz_core::session::Session::search(arg);
                if results.is_empty() {
                    println!("  {DIM}No matches for \"{arg}\"{RESET}");
                } else {
                    println!("  {BOLD}Found {} results:{RESET}", results.len());
                    for (i, r) in results.iter().enumerate() {
                        let date = r.date.get(..10).unwrap_or(&r.date);
                        println!("  {DIM}{date}{RESET} [{GOLD}{}{RESET}] ...{}...",
                            r.model,
                            r.snippet.replace('\n', " ").trim()
                        );
                        if i >= 9 { break; }
                    }
                }
            }
            CommandResult::Continue
        }

        // ── Check if it's a skill slash command ──
        _ => {
            if skill::match_skill(cmd, skills).is_some() {
                CommandResult::FallThrough
            } else {
                CommandResult::Unknown
            }
        }
    }
}

fn all_commands(skills: &[Skill]) -> Vec<(String, String)> {
    let s = t();
    let mut cmds = vec![
        ("/help".into(), s.help_help.into()),
        ("/model".into(), s.help_model.into()),
        ("/provider".into(), s.help_provider.into()),
        ("/tools".into(), s.help_tools.into()),
        ("/skills".into(), s.help_skills.into()),
        ("/clear".into(), s.help_clear.into()),
        ("/status".into(), s.help_status.into()),
        ("/undo".into(), "Undo last file edit (max 20)".into()),
        ("/compact".into(), "Compact conversation history".into()),
        ("/memory".into(), "View/add MEMORY.md entries".into()),
        ("/recall".into(), "Search across past sessions".into()),
        ("/migrate".into(), "Import config from Claude Code/Codex/Cursor".into()),
        ("/task".into(), "Task queue: list/add/done/cancel".into()),
        ("/route".into(), "Smart model routing preview".into()),
        ("/profile".into(), "Auto-detected project profile".into()),
        ("/policy".into(), "Show execution policy".into()),
        ("/guide".into(), "Quick-start guide".into()),
        ("/quit".into(), s.help_quit.into()),
    ];
    for s in skills {
        if let Some(trigger) = s.triggers.iter().find(|t| t.starts_with('/')) {
            cmds.push((trigger.clone(), s.description.clone()));
        }
    }
    cmds
}

// ── Cross-provider switch helper ──

fn switch_model_provider(
    new_model: &str,
    provider_name: Option<&str>,
    current_model: &mut String,
    current_provider: &mut Arc<dyn Provider>,
    stats: &mut status::SessionStats,
    renderer: &mut ReplRenderer,
) {
    let mut settings = Settings::load(CliOverrides::default());
    settings.model = Some(new_model.to_string());

    if let Some(pname) = provider_name {
        settings.provider = Some(pname.to_string());
    }

    match config::resolve_provider(&settings) {
        Ok(new_provider) => {
            let old_provider = current_provider.name().to_string();
            *current_model = new_model.to_string();
            *current_provider = new_provider;
            stats.model = current_model.clone();
            stats.provider = current_provider.name().to_string();

            if old_provider != current_provider.name() {
                println!("  {GREEN}●{RESET} {BOLD}{current_model}{RESET} {DIM}via{RESET} {GOLD}{}{RESET}", current_provider.name());
            } else {
                println!("  {GREEN}●{RESET} {BOLD}{current_model}{RESET}");
            }
        }
        Err(e) => {
            renderer.render_error(&format!("Cannot switch: {e}"));
        }
    }
}

/// Auto-detect provider from model name prefix
pub fn detect_provider_from_model(model: &str) -> Option<&'static str> {
    let lo = model.to_lowercase();
    if lo.starts_with("claude") {
        Some("anthropic")
    } else if lo.starts_with("gpt") || lo.starts_with("o3") || lo.starts_with("o4") || lo.starts_with("o1") {
        Some("openai")
    } else if lo.starts_with("gemini") {
        Some("gemini")
    } else if lo.starts_with("deepseek") {
        Some("deepseek")
    } else if lo.starts_with("glm") {
        Some("glm")
    } else if lo.starts_with("grok") {
        Some("grok")
    } else if lo.starts_with("llama") || lo.starts_with("qwen") || lo.starts_with("mistral") || lo.starts_with("phi") {
        Some("ollama")
    } else {
        None
    }
}

fn print_help(skills: &[Skill]) {
    let s = t();
    println!();
    println!("  {BOLD}{}{RESET}", s.help_title);
    println!("    {GOLD}/help{RESET}    {GOLD}/h{RESET}       {}", s.help_help);
    println!("    {GOLD}/model{RESET}   {GOLD}/m{RESET}       {}", s.help_model);
    println!("    {GOLD}/model{RESET}   {DIM}<name>{RESET}   {}", s.help_model_name);
    println!("    {GOLD}/provider{RESET}{GOLD}/p{RESET} {DIM}<name>{RESET}  {}", s.help_provider);
    println!("    {GOLD}/tools{RESET}   {GOLD}/t{RESET}       {}", s.help_tools);
    println!("    {GOLD}/skills{RESET}  {GOLD}/s{RESET}       {}", s.help_skills);
    println!("    {GOLD}/clear{RESET}   {GOLD}/c{RESET}       {}", s.help_clear);
    println!("    {GOLD}/status{RESET}           {}", s.help_status);
    println!("    {GOLD}/undo{RESET}    {GOLD}/u{RESET}       Undo last file edit (max 20)");
    println!("    {GOLD}/compact{RESET}          Compact conversation history");
    println!("    {GOLD}/memory{RESET}  {DIM}[text]{RESET}   View/add MEMORY.md");
    println!("    {GOLD}/recall{RESET} {DIM}<keyword>{RESET} Search past sessions");
    println!("    {GOLD}/migrate{RESET}         Import from Claude Code/Codex/Cursor");
    println!("    {GOLD}/task{RESET}    {DIM}[cmd]{RESET}    Task queue: list/add/done/cancel");
    println!("    {GOLD}/route{RESET}   {DIM}<prompt>{RESET} Smart model routing preview");
    println!("    {GOLD}/profile{RESET}          Auto-detected project profile");
    println!("    {GOLD}/policy{RESET}           Show execution policy");
    println!("    {GOLD}/guide{RESET}            Quick-start guide");
    println!("    {GOLD}/quit{RESET}    {GOLD}/q{RESET}       {}", s.help_quit);
    if !skills.is_empty() {
        println!();
        println!("  {BOLD}{}{RESET}", s.skills_title);
        for sk in skills {
            let slash = sk.triggers.iter().find(|t| t.starts_with('/')).cloned().unwrap_or_default();
            let desc = translate_skill_desc(&sk.name, &sk.description);
            println!("    {GOLD}{slash:<12}{RESET} {DIM}{desc}{RESET}");
        }
    }
    println!();
    println!("  {DIM}{}{RESET}", s.env_hint);
    println!();
}

/// Apply Pangu CJK spacing to text while preserving code blocks
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
