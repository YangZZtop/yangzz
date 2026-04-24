use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::prelude::*;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use yangzz_core::config::Settings;
use yangzz_core::config::model_meta;
use yangzz_core::memory;
use yangzz_core::message::Message;
use yangzz_core::permission::{PermissionAnswer, PermissionAsk};
use yangzz_core::provider::Provider;
use yangzz_core::query;
use yangzz_core::tool::ToolExecutor;

use super::renderer::{TuiRenderer, UiEvent};
use super::widgets::{self, ChatEntry};

const SYSTEM_PROMPT: &str = r#"You are yangzz, an AI coding assistant running in the user's terminal (TUI mode).

You have access to 14 tools: bash, file_read, file_write, file_edit, file_append, multi_edit, grep, glob, list_dir, tree, fetch, ask_user, notebook_read, notebook_edit.

Guidelines:
- Read files before editing to understand context. file_read returns the full file by default.
- Use file_edit for precise single changes (old_string must match exactly and be unique).
- Use multi_edit for multiple changes to the same file in one operation.
- Use file_write for creating new files, file_append for appending.
- Use bash for running commands, tests, installing packages.
- Use grep to search content, glob to find files by pattern, list_dir/tree for directory structure.
- Use fetch to retrieve web content.
- Use notebook_read/notebook_edit for Jupyter notebooks.
- Be concise in explanations. Show your work through tool usage.
- When editing code, preserve existing style and conventions.
- Always respond in the same language as the user's message."#;

struct AppState {
    // Chat
    entries: Vec<ChatEntry>,
    streaming_text: Option<String>,
    is_thinking: bool,
    scroll_offset: u16,

    // Input
    input: String,
    cursor_pos: usize,
    input_history: Vec<String>,
    history_index: Option<usize>,

    // Session
    messages: Vec<Message>,
    model: String,
    provider_name: String,
    total_input_tokens: u32,
    total_output_tokens: u32,
    cost_usd: f64,
    turn_count: u32,

    // Control
    should_quit: bool,
    is_processing: bool,

    // Slash command suggestion dropdown
    suggestions: Vec<(String, String)>, // (name, summary)
    suggestion_idx: usize,

    // Pending permission ask (modal dialog). When Some, keyboard is
    // intercepted to answer y/n/a.
    pending_permission: Option<PermissionAsk>,

    // Live provider + model (hot-switchable via /model command)
    provider: Arc<dyn Provider>,
    max_tokens: u32,
}

impl AppState {
    fn new(model: &str, provider: Arc<dyn Provider>, max_tokens: u32) -> Self {
        let provider_name = provider.name().to_string();
        Self {
            entries: Vec::new(),
            streaming_text: None,
            is_thinking: false,
            scroll_offset: 0,
            input: String::new(),
            cursor_pos: 0,
            input_history: Vec::new(),
            history_index: None,
            messages: Vec::new(),
            model: model.to_string(),
            provider_name,
            total_input_tokens: 0,
            total_output_tokens: 0,
            cost_usd: 0.0,
            turn_count: 0,
            should_quit: false,
            is_processing: false,
            suggestions: Vec::new(),
            suggestion_idx: 0,
            pending_permission: None,
            provider,
            max_tokens,
        }
    }

    /// Update the slash-command suggestion list based on current input.
    fn refresh_suggestions(&mut self) {
        self.suggestions.clear();
        self.suggestion_idx = 0;
        if !self.input.starts_with('/') {
            return;
        }
        // Only suggest while still typing the command name itself.
        let first_ws = self
            .input
            .find(char::is_whitespace)
            .unwrap_or(self.input.len());
        if self.cursor_pos > first_ws {
            return;
        }
        let prefix = &self.input[..first_ws];

        let registry = crate::slash::build_default();
        for cmd in registry.all() {
            let name = format!("/{}", cmd.name());
            if name.starts_with(prefix) {
                self.suggestions.push((name, cmd.summary().to_string()));
            }
        }
        // Also include legacy commands not in registry so the menu covers
        // everything the user can actually type in TUI mode.
        const LEGACY: &[(&str, &str)] = &[];
        for (n, d) in LEGACY {
            if n.starts_with(prefix) && !self.suggestions.iter().any(|(nm, _)| nm == n) {
                self.suggestions.push((n.to_string(), d.to_string()));
            }
        }
        self.suggestions.sort_by(|a, b| a.0.cmp(&b.0));
    }

    fn accept_suggestion(&mut self) {
        if let Some((name, _)) = self.suggestions.get(self.suggestion_idx).cloned() {
            // Replace the command token with the selected name + one space
            let first_ws = self
                .input
                .find(char::is_whitespace)
                .unwrap_or(self.input.len());
            let rest = self.input[first_ws..].to_string();
            self.input = format!("{name}{rest}");
            self.cursor_pos = name.len() + if rest.starts_with(' ') { 1 } else { 0 };
            if self.cursor_pos > self.input.len() {
                self.cursor_pos = self.input.len();
            }
            self.suggestions.clear();
        }
    }

    fn add_entry(&mut self, entry: ChatEntry) {
        self.entries.push(entry);
        self.scroll_offset = 0; // auto-scroll to bottom
    }

    fn add_usage(&mut self, input_tokens: u32, output_tokens: u32) {
        self.total_input_tokens += input_tokens;
        self.total_output_tokens += output_tokens;
        self.turn_count += 1;
        let (ip, op) = model_meta::lookup_model(&self.model)
            .map(|m| (m.input_price, m.output_price))
            .unwrap_or((3.0, 15.0));
        self.cost_usd += (input_tokens as f64 * ip + output_tokens as f64 * op) / 1_000_000.0;
    }

    fn total_tokens(&self) -> u32 {
        self.total_input_tokens + self.total_output_tokens
    }
}

pub async fn run(
    provider: &Arc<dyn Provider>,
    model: &str,
    max_tokens: u32,
    executor: Arc<ToolExecutor>,
    _settings: &Settings,
    permission_rx: mpsc::UnboundedReceiver<PermissionAsk>,
) -> Result<()> {
    // Setup terminal — mouse capture ON for scroll wheel.
    // Text selection: hold Shift + click/drag (standard in iTerm2,
    // Terminal.app, WezTerm, Kitty, Alacritty, Windows Terminal).
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    crate::slash::output::set_tui_mode(true);
    let result = run_app(
        &mut terminal,
        provider,
        model,
        max_tokens,
        executor,
        permission_rx,
    )
    .await;
    crate::slash::output::set_tui_mode(false);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
        crossterm::cursor::MoveTo(0, 0),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::FromCursorDown),
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    provider: &Arc<dyn Provider>,
    model: &str,
    max_tokens: u32,
    executor: Arc<ToolExecutor>,
    mut permission_rx: mpsc::UnboundedReceiver<PermissionAsk>,
) -> Result<()> {
    let mut state = AppState::new(model, Arc::clone(provider), max_tokens);

    // YANGZZ logo — pure `██` + space block style. Uses a single glyph so
    // every monospace font renders it at uniform width (the mixed ██/╗╚═
    // styles break alignment in many fonts, which is why it looked "碎碎的").
    const BANNER: &[&str] = &[
        r"██    ██   █████    ███    ██   ██████   ███████  ███████",
        r" ██  ██   ██   ██   ████   ██  ██            ██        ██",
        r"  ████    ███████   ██ ██  ██  ██   ███     ██        ██ ",
        r"   ██     ██   ██   ██  ██ ██  ██    ██    ██        ██  ",
        r"   ██     ██   ██   ██   ████   ██████   ███████  ███████",
    ];
    for line in BANNER {
        state.add_entry(ChatEntry::Banner(line.to_string()));
    }
    // spacer after the logo
    state.add_entry(ChatEntry::Banner(String::new()));

    // Welcome info — two lines with per-segment colors so version / model /
    // provider / keybinds / commands are all visually distinguishable.
    state.add_entry(ChatEntry::Custom(
        widgets::welcome_line_version_model_provider(
            env!("CARGO_PKG_VERSION"),
            model,
            provider.name(),
        ),
    ));
    state.add_entry(ChatEntry::Custom(widgets::welcome_line_hints()));

    // Channel for renderer → UI communication
    let (ui_tx, mut ui_rx) = mpsc::unbounded_channel::<UiEvent>();

    loop {
        // Process UI events from background tasks FIRST so the upcoming
        // draw() reflects the latest state immediately.
        while let Ok(ui_event) = ui_rx.try_recv() {
            handle_ui_event(&mut state, ui_event);
        }

        // Check for pending permission asks — only pick a new one if no
        // modal is currently shown.
        if state.pending_permission.is_none() {
            if let Ok(ask) = permission_rx.try_recv() {
                state.pending_permission = Some(ask);
            }
        }

        // Draw UI
        terminal.draw(|frame| draw_ui(frame, &state))?;

        // Poll events with timeout (for streaming updates)
        let timeout = if state.is_processing || state.is_thinking {
            Duration::from_millis(16) // ~60fps during streaming
        } else {
            Duration::from_millis(100)
        };

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if handle_key_event(&mut state, key, &executor, &ui_tx).await? {
                        break;
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        state.scroll_offset = state.scroll_offset.saturating_add(3);
                    }
                    MouseEventKind::ScrollDown => {
                        state.scroll_offset = state.scroll_offset.saturating_sub(3);
                    }
                    _ => {}
                },
                Event::Resize(_, _) => {
                    terminal.clear()?;
                }
                _ => {}
            }
        }

        if state.should_quit {
            break;
        }
    }

    // Save session on exit (mirrors REPL behavior)
    if !state.messages.is_empty() {
        let mut session = yangzz_core::session::Session::new(&state.model, &state.provider_name);
        session.messages = state.messages.clone();
        let _ = session.save();
    }

    Ok(())
}

fn draw_ui(frame: &mut Frame, state: &AppState) {
    let size = frame.area();

    // Layout: [Chat area] [Input 3 lines] [Status 2 lines]
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),    // chat area
            Constraint::Length(3), // input box
            Constraint::Length(2), // status bar
        ])
        .split(size);

    // Build entries including streaming text
    let mut display_entries = state.entries.clone();
    if let Some(ref text) = state.streaming_text {
        display_entries.push(ChatEntry::Assistant(text.clone()));
    }

    // Chat area
    widgets::render_chat(
        frame,
        chunks[0],
        &display_entries,
        state.scroll_offset,
        state.is_thinking,
    );

    // Input
    let input_label = if state.is_processing { "⏳" } else { "" };
    widgets::render_input(
        frame,
        chunks[1],
        &state.input,
        state.cursor_pos,
        input_label,
    );

    // Status bar
    widgets::render_status_bar(
        frame,
        chunks[2],
        &state.model,
        &state.provider_name,
        state.total_tokens(),
        state.cost_usd,
    );

    // Suggestion dropdown (above input area when slash command is being typed)
    if !state.suggestions.is_empty() {
        widgets::render_suggestions(frame, chunks[1], &state.suggestions, state.suggestion_idx);
    }

    // Permission modal — centered overlay when a tool is waiting for approval
    if let Some(ref ask) = state.pending_permission {
        widgets::render_permission_modal(frame, size, ask);
    }
}

fn handle_ui_event(state: &mut AppState, event: UiEvent) {
    match event {
        UiEvent::TextDelta(text) => {
            state.streaming_text = Some(text);
        }
        UiEvent::AssistantComplete(text) => {
            state.streaming_text = None;
            state.add_entry(ChatEntry::Assistant(text));
        }
        UiEvent::ToolStart { name } => {
            state.add_entry(ChatEntry::Info(format!("⚙ {name}...")));
        }
        UiEvent::ToolResult {
            name,
            result,
            is_error,
        } => {
            state.add_entry(ChatEntry::Tool {
                name,
                result,
                is_error,
            });
        }
        UiEvent::Error(msg) => {
            state.is_thinking = false;
            state.add_entry(ChatEntry::Error(msg));
        }
        UiEvent::Info(msg) => {
            state.add_entry(ChatEntry::Info(msg));
        }
        UiEvent::ThinkingStart => {
            state.is_thinking = true;
        }
        UiEvent::ThinkingStop => {
            state.is_thinking = false;
        }
        UiEvent::ResponseComplete => {
            state.is_processing = false;
            state.is_thinking = false;
            state.streaming_text = None;
        }
        UiEvent::UsageUpdate {
            input_tokens,
            output_tokens,
        } => {
            state.add_usage(input_tokens, output_tokens);
        }
        UiEvent::MessagesSync(msgs) => {
            state.messages = msgs;
        }
    }
}

async fn handle_key_event(
    state: &mut AppState,
    key: KeyEvent,
    executor: &Arc<ToolExecutor>,
    ui_tx: &mpsc::UnboundedSender<UiEvent>,
) -> Result<bool> {
    // Permission modal is up — intercept all keys for y/n/a answering.
    if state.pending_permission.is_some() {
        let answer = match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Some(PermissionAnswer::Yes),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Some(PermissionAnswer::No),
            KeyCode::Char('a') | KeyCode::Char('A') => Some(PermissionAnswer::Always),
            _ => None,
        };
        if let Some(ans) = answer {
            let ask = state.pending_permission.take().unwrap();
            let label = match ans {
                PermissionAnswer::Yes => "允许",
                PermissionAnswer::No => "拒绝",
                PermissionAnswer::Always => "始终允许",
            };
            state.add_entry(ChatEntry::Info(format!("🔐 {} · {}", ask.tool_name, label)));
            let _ = ask.reply.send(ans);
        }
        return Ok(false);
    }

    // Ctrl+D = quit
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('d') {
        return Ok(true);
    }

    // Ctrl+C = cancel current or clear input
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        if state.is_processing {
            state.is_processing = false;
            state.is_thinking = false;
            state.streaming_text = None;
            state.add_entry(ChatEntry::Info("(cancelled)".into()));
        } else {
            state.input.clear();
            state.cursor_pos = 0;
        }
        return Ok(false);
    }

    // Don't accept input while processing
    if state.is_processing {
        return Ok(false);
    }

    match key.code {
        // Tab = accept highlighted suggestion (while suggestion menu is open)
        KeyCode::Tab if !state.suggestions.is_empty() => {
            state.accept_suggestion();
            return Ok(false);
        }
        // Esc = dismiss suggestion menu
        KeyCode::Esc if !state.suggestions.is_empty() => {
            state.suggestions.clear();
            return Ok(false);
        }
        // Arrow keys navigate suggestion menu when it's open
        KeyCode::Up if !state.suggestions.is_empty() => {
            if state.suggestion_idx > 0 {
                state.suggestion_idx -= 1;
            }
            return Ok(false);
        }
        KeyCode::Down if !state.suggestions.is_empty() => {
            if state.suggestion_idx + 1 < state.suggestions.len() {
                state.suggestion_idx += 1;
            }
            return Ok(false);
        }
        KeyCode::Enter => {
            let input = state.input.trim().to_string();
            if input.is_empty() {
                return Ok(false);
            }

            // Save to history
            state.input_history.push(input.clone());
            state.history_index = None;
            state.input.clear();
            state.cursor_pos = 0;
            state.suggestions.clear();

            // v0.3.0 slash registry dispatch (captures output to text buffer)
            if input.starts_with('/') {
                if let Some(outcome) = try_dispatch_slash(state, &input, executor.as_ref()) {
                    if matches!(outcome, crate::slash::Outcome::Quit) {
                        return Ok(true);
                    }
                    return Ok(false);
                }
            }

            // Unknown slash command — DON'T send to the LLM as chat.
            // Give a clear error so the user knows their command was
            // misspelled or missing.
            if input.starts_with('/') {
                state.add_entry(ChatEntry::Error(format!(
                    "未知命令 {input}  ·  /help 查看所有命令"
                )));
                return Ok(false);
            }

            // Parse `@path/img.png` attachments — user sees a summary in
            // the chat, image bytes are embedded into the Message content.
            let parsed = yangzz_core::attach::parse_input(&input);
            for w in &parsed.warnings {
                state.add_entry(ChatEntry::Error(w.clone()));
            }
            // Add user message (show original input text for clarity)
            state.add_entry(ChatEntry::User(input.clone()));
            if !parsed.attachments_summary.is_empty() {
                state.add_entry(ChatEntry::Info(parsed.attachments_summary.clone()));
            }
            state.messages.push(parsed.message);
            state.is_processing = true;

            // Spawn the agentic loop
            let provider = Arc::clone(&state.provider);
            let model = state.model.clone();
            let max_tokens = state.max_tokens;
            let tx = ui_tx.clone();
            let mut messages = state.messages.clone();
            let executor = Arc::clone(executor);
            let user_input = input.clone(); // for auto memory capture

            tokio::spawn(async move {
                let mut renderer = TuiRenderer::new(tx.clone());
                let cwd = std::env::current_dir().unwrap_or_default();
                let system = memory::inject_memory_prompt(SYSTEM_PROMPT, &cwd);
                let result = query::run_agentic_loop(
                    &provider,
                    &model,
                    max_tokens,
                    &mut messages,
                    Some(system),
                    &executor,
                    &mut renderer,
                )
                .await;

                match result {
                    Ok(usage) => {
                        // Auto memory capture — extract memory-worthy signals
                        // from the last assistant response (mirrors REPL logic).
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
                        memory::auto_capture(&user_input, &assistant_text, &cwd);

                        // Sync messages back (includes assistant responses + tool results)
                        let _ = tx.send(UiEvent::MessagesSync(messages));
                        let _ = tx.send(UiEvent::UsageUpdate {
                            input_tokens: usage.input_tokens,
                            output_tokens: usage.output_tokens,
                        });
                        let _ = tx.send(UiEvent::ResponseComplete);
                    }
                    Err(e) => {
                        let msg = format!("{e}");
                        if msg.contains("Cancelled by user") {
                            let _ = tx.send(UiEvent::Info("(cancelled)".into()));
                        } else {
                            let _ = tx.send(UiEvent::Error(msg));
                        }
                        // Still sync messages (some may have been added before error)
                        let _ = tx.send(UiEvent::MessagesSync(messages));
                        let _ = tx.send(UiEvent::ResponseComplete);
                    }
                }
            });
        }
        KeyCode::Char(c) => {
            // cursor_pos is a BYTE index (required by String::insert). For
            // multi-byte chars (CJK), advance by c.len_utf8() so we stay on
            // a char boundary.
            state.input.insert(state.cursor_pos, c);
            state.cursor_pos += c.len_utf8();
            state.refresh_suggestions();
        }
        KeyCode::Backspace => {
            if state.cursor_pos > 0 {
                // Walk back to the previous char boundary
                let prev = prev_char_boundary(&state.input, state.cursor_pos);
                state.input.replace_range(prev..state.cursor_pos, "");
                state.cursor_pos = prev;
            }
            state.refresh_suggestions();
        }
        KeyCode::Delete => {
            if state.cursor_pos < state.input.len() {
                let next = next_char_boundary(&state.input, state.cursor_pos);
                state.input.replace_range(state.cursor_pos..next, "");
            }
            state.refresh_suggestions();
        }
        KeyCode::Left => {
            if state.cursor_pos > 0 {
                state.cursor_pos = prev_char_boundary(&state.input, state.cursor_pos);
            }
        }
        KeyCode::Right => {
            if state.cursor_pos < state.input.len() {
                state.cursor_pos = next_char_boundary(&state.input, state.cursor_pos);
            }
        }
        KeyCode::Home => {
            state.cursor_pos = 0;
        }
        KeyCode::End => {
            state.cursor_pos = state.input.len();
        }
        KeyCode::Up => {
            // History navigation
            if !state.input_history.is_empty() {
                let idx = match state.history_index {
                    Some(0) => 0,
                    Some(i) => i - 1,
                    None => state.input_history.len() - 1,
                };
                state.history_index = Some(idx);
                state.input = state.input_history[idx].clone();
                state.cursor_pos = state.input.len();
            }
        }
        KeyCode::Down => {
            if let Some(idx) = state.history_index {
                if idx + 1 < state.input_history.len() {
                    state.history_index = Some(idx + 1);
                    state.input = state.input_history[idx + 1].clone();
                    state.cursor_pos = state.input.len();
                } else {
                    state.history_index = None;
                    state.input.clear();
                    state.cursor_pos = 0;
                }
            }
        }
        KeyCode::PageUp => {
            // Go back in history (scroll up → bigger offset-from-bottom).
            state.scroll_offset = state.scroll_offset.saturating_add(10);
        }
        KeyCode::PageDown => {
            state.scroll_offset = state.scroll_offset.saturating_sub(10);
        }
        _ => {}
    }

    Ok(false)
}

/// Dispatch a slash command via the v0.3.0 registry, capturing its output
/// into a single ChatEntry::Info so it shows in the TUI history instead of
/// writing to stdout (which would break raw-mode rendering).
///
/// Returns Some(outcome) if the registry handled it, None if the caller
/// should fall through to legacy handlers.
fn try_dispatch_slash(
    state: &mut AppState,
    line: &str,
    executor: &ToolExecutor,
) -> Option<crate::slash::Outcome> {
    use crate::slash::{CommandContext, Outcome};
    use crate::ui::status::SessionStats;

    let registry = crate::slash::build_default();

    // Quick peek: is the command name known to the registry?
    let name = line
        .trim_start_matches('/')
        .split_whitespace()
        .next()
        .unwrap_or("");
    if registry.find(name).is_none() {
        return None;
    }

    // Build CommandContext backed by state — mutations propagated back.
    let mut local_provider = Arc::clone(&state.provider);
    let mut local_model = state.model.clone();
    let mut local_stats = SessionStats {
        model: state.model.clone(),
        provider: state.provider_name.clone(),
        total_input_tokens: state.total_input_tokens,
        total_output_tokens: state.total_output_tokens,
        total_cost_usd: state.cost_usd,
        total_turns: state.turn_count,
    };
    let settings = yangzz_core::config::Settings::load(Default::default());
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut skills = yangzz_core::skill::builtin_skills();
    skills.extend(yangzz_core::skill::load_skills(&cwd));

    let mut ctx = CommandContext {
        current_model: &mut local_model,
        current_provider: &mut local_provider,
        messages: &mut state.messages,
        stats: &mut local_stats,
        settings: &settings,
        executor,
        skills: &skills,
    };

    // Capture output and outcome in a single dispatch — no side-effect
    // duplication.
    let (output, outcome) =
        crate::slash::output::with_capture(|| registry.dispatch(&mut ctx, line));

    // If the registered command explicitly refused (Unhandled), return None
    // so the caller falls through to legacy TUI handlers.
    if matches!(outcome, Outcome::Unhandled) {
        return None;
    }

    let normalized_name = name.to_ascii_lowercase();

    if normalized_name == "clear" || normalized_name == "c" {
        reset_tui_conversation(state);
    }

    // Propagate any provider/model changes from the command back to state
    if local_model != state.model || local_provider.name() != state.provider_name {
        state.model = local_model;
        state.provider_name = local_provider.name().to_string();
        state.provider = local_provider;
    }

    let trimmed = output.trim_end();
    if !trimmed.is_empty() {
        // Strip ANSI color codes — ratatui Paragraph shows them literally
        // (it doesn't interpret terminal escape sequences within text).
        let clean = strip_ansi(trimmed);
        state.add_entry(ChatEntry::Info(clean));
    }

    Some(outcome)
}

fn reset_tui_conversation(state: &mut AppState) {
    state.entries.clear();
    state.total_input_tokens = 0;
    state.total_output_tokens = 0;
    state.cost_usd = 0.0;
    state.turn_count = 0;
    state.scroll_offset = 0;
    state.streaming_text = None;
    state.is_processing = false;
    state.is_thinking = false;

    const BANNER: &[&str] = &[
        r"██    ██   █████    ███    ██   ██████   ███████  ███████",
        r" ██  ██   ██   ██   ████   ██  ██            ██        ██",
        r"  ████    ███████   ██ ██  ██  ██   ███     ██        ██ ",
        r"   ██     ██   ██   ██  ██ ██  ██    ██    ██        ██  ",
        r"   ██     ██   ██   ██   ████   ██████   ███████  ███████",
    ];
    for line in BANNER {
        state.entries.push(ChatEntry::Banner(line.to_string()));
    }
    state.entries.push(ChatEntry::Banner(String::new()));
    state.entries.push(ChatEntry::Custom(
        widgets::welcome_line_version_model_provider(
            env!("CARGO_PKG_VERSION"),
            &state.model,
            &state.provider_name,
        ),
    ));
    state
        .entries
        .push(ChatEntry::Custom(widgets::welcome_line_hints()));
}

/// Find the byte index of the previous char boundary before `byte_idx`.
/// Safe to call at any byte_idx <= s.len().
fn prev_char_boundary(s: &str, byte_idx: usize) -> usize {
    if byte_idx == 0 {
        return 0;
    }
    let mut i = byte_idx - 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Find the byte index of the next char boundary after `byte_idx`.
fn next_char_boundary(s: &str, byte_idx: usize) -> usize {
    let len = s.len();
    if byte_idx >= len {
        return len;
    }
    let mut i = byte_idx + 1;
    while i < len && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Remove ANSI CSI escape sequences (like \x1b[38;5;178m) from a string so
/// it renders as plain text in ratatui (which doesn't interpret them).
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until the final byte of a CSI sequence (alpha char) or end
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                for nc in chars.by_ref() {
                    if nc.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                // Other escape forms — skip one char and continue
                chars.next();
            }
            continue;
        }
        out.push(c);
    }
    out
}
