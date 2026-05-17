use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::{Config, Editor};
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Instant;
use yangzz_core::config;
use yangzz_core::config::Settings;
use yangzz_core::config::settings::CliOverrides;
use yangzz_core::db::Database;
use yangzz_core::memory;
use yangzz_core::message::Message;
use yangzz_core::provider::Provider;
use yangzz_core::query;
use yangzz_core::render::Renderer;
use yangzz_core::session::Session;
use yangzz_core::skill::{self};
use yangzz_core::tool::ToolExecutor;

use crate::repl_commands::{CommandResult, handle_command, switch_model_provider};
use crate::repl_help::all_commands;
use crate::repl_render::ReplRenderer;
use crate::ui::format::*;
use crate::ui::i18n::t;
use crate::ui::{banner, select, status};

const SYSTEM_PROMPT: &str = r#"You are yangzz, an AI coding assistant running in the user's terminal.

You have access to these tools:
bash, file_read, file_write, file_edit, file_append, multi_edit, parallel_edit,
grep, glob, list_dir, tree, fetch, web_search, browser, ask_user, notebook_read,
notebook_edit, code_graph, sub_agent, todo.

Guidelines:
- Read files before editing to understand context. file_read returns the full file by default.
- Use file_edit for precise single changes (old_string must match exactly and be unique).
- Use multi_edit for multiple changes to the same file in one operation.
- Use file_write for creating new files, file_append for appending.
- Use bash for running commands, tests, installing packages.
- Use grep to search content, glob to find files by pattern, list_dir/tree for directory structure.
- Prefer code_graph over grep/bash when the user asks structural questions about code — e.g.
  "how many structs / classes / traits / functions", "where is X defined", "who calls X",
  "list symbols in file Y". code_graph uses tree-sitter AST parsing (Rust/TS/TSX/Python)
  so it's faster and more accurate than text-level search, and it already excludes
  node_modules, venv, target, .git, dist, build.
- Use fetch to retrieve raw content from a specific URL.
- Use web_search to search the internet for documentation, solutions, or current information.
- Use browser to open a URL and read its content in clean text (like reader mode).
  Prefer browser over fetch when you need to read a web page's text content.
- Use ask_user when you need clarification from the user.
- Use notebook_read/notebook_edit for Jupyter notebooks.
- Be concise in explanations. Show your work through tool usage.
- When editing code, preserve existing style and conventions.
- Always respond in the same language as the user's message."#;

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
        provider,
        model,
        max_tokens,
        &mut messages,
        Some(SYSTEM_PROMPT.to_string()),
        executor,
        &mut renderer,
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

    // SQLite persistence
    let db = {
        let db_path = Database::default_path();
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        Database::open(&db_path).ok()
    };
    if let Some(ref db) = db {
        let _ = db.create_session(
            &session.id,
            &current_model,
            provider.name(),
            &session.cwd,
        );
    }

    // Skills
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut skills = skill::builtin_skills();
    skills.extend(skill::load_skills(&cwd));

    // ── Welcome ──
    banner::print_welcome(&current_model, provider.name(), env!("CARGO_PKG_VERSION"));

    // Initialize runtime thinking from config
    yangzz_core::config::settings::init_runtime_thinking(settings);

    // ── Auto-resume: offer to continue recent session ──
    if let Some(prev) = Session::load_latest() {
        if !prev.messages.is_empty() {
            if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&prev.updated_at) {
                let age = chrono::Utc::now().signed_duration_since(ts);
                if age.num_hours() < 4 {
                    let msg_count = prev.messages.len();
                    let ago = if age.num_minutes() < 1 {
                        "just now".to_string()
                    } else if age.num_minutes() < 60 {
                        format!("{}m ago", age.num_minutes())
                    } else {
                        format!("{}h ago", age.num_hours())
                    };
                    println!(
                        "  {DIM}Previous session ({msg_count} messages, {ago}){RESET}"
                    );
                    println!("  {SOFT_GOLD}Resume? [y/N]{RESET} ");
                    let _ = io::stdout().flush();
                    // Use a simple non-blocking approach: read one line from stdin
                    // with a clear visual prompt so user knows we're waiting
                    let mut answer = String::new();
                    if io::stdin().read_line(&mut answer).is_ok()
                        && matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
                    {
                        messages = prev.messages.clone();
                        session = prev;
                        println!("  {GREEN}✓{RESET} {DIM}Resumed ({msg_count} messages){RESET}");
                    } else {
                        println!("  {DIM}Starting fresh session{RESET}");
                    }
                }
            }
        }
    }

    let config = Config::builder()
        .completion_type(rustyline::CompletionType::List)
        .build();
    let mut rl: Editor<crate::slash::readline_helper::YangzzHelper, FileHistory> =
        Editor::with_config(config).expect("Failed to init readline");
    rl.set_helper(Some(crate::slash::readline_helper::YangzzHelper::new()));

    // Persist input history across sessions
    let history_path = yangzz_core::paths::yangzz_dir().join("history");
    let _ = rl.load_history(&history_path);
    // Gold chip ❯ + light warm background for input area (was 236, now 238 + white text)
    let prompt = format!("\x1b[48;5;178m\x1b[1;30m ❯ \x1b[0m ");
    let prompt_cont = format!("    ");
    loop {
        // Breathing room: blank line before every prompt so consecutive
        // turns don't visually fuse into a wall of text.
        println!();

        // Multi-line input: end line with \ to continue
        let mut full_input = String::new();
        let mut is_continuation = false;
        loop {
            let p = if is_continuation {
                &prompt_cont
            } else {
                &prompt
            };
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
                    let _ = rl.save_history(&history_path);
                    return Ok(()); // exit REPL
                }
                Err(_) => {
                    let _ = rl.save_history(&history_path);
                    return Ok(());
                }
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

            // /model with no args is handled here (async) to fetch models
            // from ALL available providers for the interactive picker.
            if (cmd == "/model" || cmd == "/m") && arg.is_empty() {
                let msg_count = messages.len();
                let live_settings = Settings::load(CliOverrides::default());
                // Fetch models from ALL available providers
                print!("  {DIM}{}{RESET}", t().fetching_models);
                let _ = io::stdout().flush();

                let available = config::retain_configured_providers(
                    config::list_available_providers(Some(&current_provider), &live_settings),
                    &live_settings,
                );
                let mut provider_models: Vec<select::ProviderModels> = Vec::new();
                for ap in &available {
                    // Timeout each provider's list_models to 10s — don't let a slow
                    // provider block the entire picker
                    let models = match tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        ap.provider.list_models(),
                    )
                    .await
                    {
                        Ok(Ok(models)) if !models.is_empty() => models,
                        _ => config::fallback_models_for_provider(
                            &live_settings,
                            &ap.name,
                            ap.provider.default_model(),
                        ),
                    };
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
                            &sel_model,
                            Some(&sel_provider),
                            &mut current_model,
                            &mut current_provider,
                            &mut stats,
                            &mut renderer,
                        );
                        if msg_count > 0 {
                            println!(
                                "  {DIM}{}{RESET}",
                                t().history_kept.replace("{}", &msg_count.to_string())
                            );
                        }
                    }
                }
                continue;
            }

            let handled = handle_command(
                cmd,
                arg,
                &mut current_model,
                &mut current_provider,
                &mut messages,
                &mut stats,
                executor,
                &skills,
                settings,
            );

            match handled {
                CommandResult::Continue => continue,
                CommandResult::Quit => {
                    session.messages = messages.clone();
                    let _ = session.save();
                    let _ = rl.save_history(&history_path);
                    break;
                }
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
            format!(
                "{}\n\n--- Active Skill: {} ---\n{}",
                base_system, matched.name, matched.body
            )
        } else {
            base_system
        };

        println!();

        // ── Check for team directives (e.g. "Claude写前端，GPT写后端，帮我重构") ──
        let directive_result = yangzz_core::provider::router::parse_directives(input);

        if directive_result.has_directives && settings.providers.len() > 1 {
            // User gave natural language team assignments!
            println!("  {BOLD_GOLD}🏗 Team mode detected:{RESET}");
            for d in &directive_result.directives {
                println!(
                    "    {GOLD}{}{RESET} → {BOLD}{}{RESET}",
                    d.domain.as_str(),
                    d.model_hint
                );
            }
            println!("  {DIM}Task: {}{RESET}", directive_result.task);
            println!();

            let strategy =
                yangzz_core::provider::router::directives_to_strategy(&directive_result.directives);

            // Build provider list: match model hints to configured providers
            let available_providers: Vec<(String, Arc<dyn Provider>, String)> = settings
                .providers
                .iter()
                .filter_map(|ep| {
                    // Wrap ExtraProvider as a Settings to call resolve_provider
                    let tmp_settings = Settings {
                        provider: Some(ep.name.clone()),
                        api_key: Some(ep.api_key.clone()),
                        base_url: Some(ep.base_url.clone()),
                        api_format: ep.api_format.clone(),
                        providers: vec![ep.clone()],
                        ..Settings::default()
                    };
                    let provider_arc = match config::resolve_provider(&tmp_settings) {
                        Ok(p) => p,
                        Err(_) => return None,
                    };
                    let model = ep.default_model.clone().unwrap_or_default();
                    Some((ep.name.clone(), provider_arc, model))
                })
                .collect();

            // Map model hints (claude, openai) to actual provider names
            let mapped_providers: Vec<(String, Arc<dyn Provider>, String)> = directive_result
                .directives
                .iter()
                .filter_map(|d| {
                    // Find a provider whose name contains the model hint
                    available_providers
                        .iter()
                        .find(|(name, _, model)| {
                            let lower_name = name.to_lowercase();
                            let lower_model = model.to_lowercase();
                            lower_name.contains(&d.model_hint)
                                || lower_model.contains(&d.model_hint)
                        })
                        .cloned()
                })
                .collect();

            if !mapped_providers.is_empty() {
                messages.push(Message::user(&directive_result.task));
                let start = Instant::now();
                let result = yangzz_core::team::execute_with_strategy(
                    &directive_result.task,
                    &strategy,
                    &mapped_providers,
                    executor,
                    &mut renderer,
                    max_tokens,
                )
                .await;
                let elapsed = start.elapsed().as_secs_f64();

                match result {
                    Ok(_response) => {
                        status::render_turn_info(elapsed);
                        status::render_status_bar(&stats);
                    }
                    Err(e) => {
                        renderer.render_error(&format!("{e}"));
                    }
                }
                crate::ui::format::print_divider();
                continue;
            } else {
                println!("  {DIM}(providers not matched, falling back to single model){RESET}");
            }
        }

        // Parse `@path/img.png` attachments before sending.
        let parsed = yangzz_core::attach::parse_input(input);
        for w in &parsed.warnings {
            println!("  {YELLOW}⚠{RESET} {w}");
        }
        if !parsed.attachments_summary.is_empty() {
            println!("  {SOFT_GOLD}{}{RESET}", parsed.attachments_summary);
        }
        messages.push(parsed.message);

        // ── Run agentic loop ──
        let start = Instant::now();
        let result = query::run_agentic_loop(
            &current_provider,
            &current_model,
            max_tokens,
            &mut messages,
            Some(system),
            executor,
            &mut renderer,
        )
        .await;
        let elapsed = start.elapsed().as_secs_f64();

        match result {
            Ok(usage) => {
                stats.add_usage(usage.input_tokens, usage.output_tokens);
                status::render_turn_info(elapsed);
                status::render_status_bar(&stats);
                crate::ui::format::print_divider();

                // Auto-save session after every successful turn
                session.messages = messages.clone();
                let _ = session.save();

                // Persist to SQLite: save the last user + assistant messages
                if let Some(ref db) = db {
                    let recent: Vec<&Message> = messages.iter().rev().take(2).collect();
                    for msg in recent.iter().rev() {
                        let role = match msg.role {
                            yangzz_core::message::Role::User => "user",
                            yangzz_core::message::Role::Assistant => "assistant",
                            yangzz_core::message::Role::System => "system",
                        };
                        if let Ok(json) = serde_json::to_string(&msg.content) {
                            let _ = db.insert_message(&session.id, role, &json);
                        }
                    }

                    // Auto-title: generate session title after 5 user messages
                    let user_msg_count = messages
                        .iter()
                        .filter(|m| m.role == yangzz_core::message::Role::User)
                        .count();
                    if user_msg_count == 5 {
                        let summary_input = build_title_prompt(&messages);
                        let provider_clone = Arc::clone(&current_provider);
                        let model_clone = current_model.clone();
                        let session_id = session.id.clone();
                        let db_path = Database::default_path();
                        tokio::spawn(async move {
                            if let Ok(title) = generate_title(
                                &provider_clone,
                                &model_clone,
                                &summary_input,
                            )
                            .await
                            {
                                if let Ok(db) = Database::open(&db_path) {
                                    let _ = db.update_session_title(&session_id, &title);
                                }
                            }
                        });
                    }
                }

                // Auto memory capture: scan the last exchange for memory-worthy signals
                let assistant_text = messages
                    .iter()
                    .rev()
                    .find_map(|m| {
                        m.content.iter().find_map(|b| {
                            if let yangzz_core::message::ContentBlock::Text { text } = b {
                                Some(text.clone())
                            } else {
                                None
                            }
                        })
                    })
                    .unwrap_or_default();
                memory::auto_capture(input, &assistant_text, &cwd);
            }
            Err(e) => {
                renderer.stop_spinner();
                let msg = format!("{e}");
                if msg.contains("Cancelled by user") {
                    println!("\n  {DIM}(cancelled){RESET}");
                } else {
                    renderer.render_error(&msg);
                }
                crate::ui::format::print_divider();
                messages.pop();
            }
        }
    }

    Ok(())
}

fn build_title_prompt(messages: &[Message]) -> String {
    let mut snippets = String::new();
    for msg in messages.iter().take(10) {
        let role = match msg.role {
            yangzz_core::message::Role::User => "U",
            yangzz_core::message::Role::Assistant => "A",
            yangzz_core::message::Role::System => continue,
        };
        for block in &msg.content {
            if let yangzz_core::message::ContentBlock::Text { text } = block {
                let short: String = text.chars().take(100).collect();
                snippets.push_str(&format!("{role}: {short}\n"));
                break;
            }
        }
    }
    snippets
}

async fn generate_title(
    provider: &Arc<dyn Provider>,
    model: &str,
    conversation_snippet: &str,
) -> Result<String> {
    use yangzz_core::provider::CreateMessageRequest;

    let prompt = format!(
        "Based on this conversation, generate a short title (max 40 chars, same language as the user). Reply with ONLY the title, nothing else.\n\n{conversation_snippet}"
    );
    let request = CreateMessageRequest {
        model: model.to_string(),
        messages: vec![Message::user(&prompt)],
        system: None,
        max_tokens: 60,
        temperature: Some(0.3),
        tools: vec![],
        thinking_budget: None,
        reasoning_effort: None,
    };
    let response = provider.create_message(&request).await?;
    let title = response
        .message
        .content
        .iter()
        .find_map(|b| {
            if let yangzz_core::message::ContentBlock::Text { text } = b {
                Some(text.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();
    Ok(title)
}