use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::prelude::*;
use ratatui::Terminal;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use yangzz_core::config::Settings;
use yangzz_core::config::model_meta;
use yangzz_core::memory;
use yangzz_core::message::Message;
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
}

impl AppState {
    fn new(model: &str, provider_name: &str) -> Self {
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
            provider_name: provider_name.to_string(),
            total_input_tokens: 0,
            total_output_tokens: 0,
            cost_usd: 0.0,
            turn_count: 0,
            should_quit: false,
            is_processing: false,
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
) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, provider, model, max_tokens, executor).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    provider: &Arc<dyn Provider>,
    model: &str,
    max_tokens: u32,
    executor: Arc<ToolExecutor>,
) -> Result<()> {
    let mut state = AppState::new(model, provider.name());

    // Welcome entry
    state.add_entry(ChatEntry::Info(format!(
        "yangzz v{} — {} via {}",
        env!("CARGO_PKG_VERSION"),
        model,
        provider.name()
    )));
    state.add_entry(ChatEntry::Info(
        "Ctrl+D exit · /help commands · /clear reset · PageUp/Down scroll".into(),
    ));

    // Channel for renderer → UI communication
    let (ui_tx, mut ui_rx) = mpsc::unbounded_channel::<UiEvent>();

    loop {
        // Draw UI
        terminal.draw(|frame| draw_ui(frame, &state))?;

        // Poll events with timeout (for streaming updates)
        let timeout = if state.is_processing || state.is_thinking {
            Duration::from_millis(16) // ~60fps during streaming
        } else {
            Duration::from_millis(100)
        };

        // Check for UI events from renderer
        while let Ok(ui_event) = ui_rx.try_recv() {
            handle_ui_event(&mut state, ui_event);
        }

        // Check for keyboard events
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if handle_key_event(&mut state, key, provider, model, max_tokens, &executor, &ui_tx).await? {
                    break;
                }
            }
        }

        if state.should_quit {
            break;
        }
    }

    Ok(())
}

fn draw_ui(frame: &mut Frame, state: &AppState) {
    let size = frame.area();

    // Layout: [Chat area] [Input 3 lines] [Status 2 lines]
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),        // chat area
            Constraint::Length(3),      // input box
            Constraint::Length(2),      // status bar
        ])
        .split(size);

    // Build entries including streaming text
    let mut display_entries = state.entries.clone();
    if let Some(ref text) = state.streaming_text {
        display_entries.push(ChatEntry::Assistant(text.clone()));
    }

    // Chat area
    widgets::render_chat(frame, chunks[0], &display_entries, state.scroll_offset, state.is_thinking);

    // Input
    let input_label = if state.is_processing { "⏳" } else { "" };
    widgets::render_input(frame, chunks[1], &state.input, state.cursor_pos, input_label);

    // Status bar
    widgets::render_status_bar(
        frame,
        chunks[2],
        &state.model,
        &state.provider_name,
        state.total_tokens(),
        state.cost_usd,
    );
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
        UiEvent::ToolResult { name, result, is_error } => {
            state.add_entry(ChatEntry::Tool { name, result, is_error });
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
        UiEvent::UsageUpdate { input_tokens, output_tokens } => {
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
    provider: &Arc<dyn Provider>,
    model: &str,
    max_tokens: u32,
    executor: &Arc<ToolExecutor>,
    ui_tx: &mpsc::UnboundedSender<UiEvent>,
) -> Result<bool> {
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

            // Handle slash commands
            if input == "/quit" || input == "/exit" {
                return Ok(true);
            }
            if input == "/clear" {
                state.entries.clear();
                state.messages.clear();
                state.total_input_tokens = 0;
                state.total_output_tokens = 0;
                state.cost_usd = 0.0;
                state.turn_count = 0;
                state.add_entry(ChatEntry::Info("Conversation cleared.".into()));
                return Ok(false);
            }
            if input == "/help" {
                state.add_entry(ChatEntry::Info(
                    "Commands: /quit /clear /help · Ctrl+C cancel · Ctrl+D exit · PageUp/Down scroll"
                        .into(),
                ));
                return Ok(false);
            }
            if input == "/status" {
                state.add_entry(ChatEntry::Info(format!(
                    "Model: {} · Provider: {} · Tokens: {} in + {} out · Cost: ${:.4} · Turns: {}",
                    state.model,
                    state.provider_name,
                    state.total_input_tokens,
                    state.total_output_tokens,
                    state.cost_usd,
                    state.turn_count,
                )));
                return Ok(false);
            }
            if input == "/undo" {
                let msg = executor.undo().await.unwrap_or_else(|| "Nothing to undo".into());
                state.add_entry(ChatEntry::Info(format!("↩ {msg}")));
                return Ok(false);
            }
            if input == "/compact" {
                let before = state.messages.len();
                query::compact_messages_public(&mut state.messages);
                let after = state.messages.len();
                state.add_entry(ChatEntry::Info(format!("Compacted: {before} → {after} messages")));
                return Ok(false);
            }
            if input.starts_with("/memory") {
                let cwd = std::env::current_dir().unwrap_or_default();
                let arg = input.strip_prefix("/memory").unwrap_or("").trim();
                if arg.is_empty() {
                    match memory::load_memory(&cwd) {
                        Some(mem) => state.add_entry(ChatEntry::Info(mem)),
                        None => state.add_entry(ChatEntry::Info("No MEMORY.md found".into())),
                    }
                } else {
                    match memory::append_memory(&cwd, arg) {
                        Ok(()) => state.add_entry(ChatEntry::Info("Saved to MEMORY.md".into())),
                        Err(e) => state.add_entry(ChatEntry::Error(e)),
                    }
                }
                return Ok(false);
            }

            // Add user message
            state.add_entry(ChatEntry::User(input.clone()));
            state.messages.push(Message::user(&input));
            state.is_processing = true;

            // Spawn the agentic loop
            let provider = Arc::clone(provider);
            let model = model.to_string();
            let tx = ui_tx.clone();
            let mut messages = state.messages.clone();
            let executor = Arc::clone(executor);

            tokio::spawn(async move {
                let mut renderer = TuiRenderer::new(tx.clone());
                let cwd = std::env::current_dir().unwrap_or_default();
                let system = memory::inject_memory_prompt(SYSTEM_PROMPT, &cwd);
                let result = query::run_agentic_loop(
                    &provider, &model, max_tokens,
                    &mut messages, Some(system),
                    &executor, &mut renderer,
                )
                .await;

                match result {
                    Ok(usage) => {
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
            state.input.insert(state.cursor_pos, c);
            state.cursor_pos += 1;
        }
        KeyCode::Backspace => {
            if state.cursor_pos > 0 {
                state.cursor_pos -= 1;
                state.input.remove(state.cursor_pos);
            }
        }
        KeyCode::Delete => {
            if state.cursor_pos < state.input.len() {
                state.input.remove(state.cursor_pos);
            }
        }
        KeyCode::Left => {
            if state.cursor_pos > 0 {
                state.cursor_pos -= 1;
            }
        }
        KeyCode::Right => {
            if state.cursor_pos < state.input.len() {
                state.cursor_pos += 1;
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
            state.scroll_offset = state.scroll_offset.saturating_add(10);
        }
        KeyCode::PageDown => {
            state.scroll_offset = state.scroll_offset.saturating_sub(10);
        }
        _ => {}
    }

    Ok(false)
}
