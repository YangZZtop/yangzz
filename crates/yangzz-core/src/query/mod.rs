use crate::message::{ContentBlock, Message, Role, Usage};
use crate::provider::{
    CreateMessageRequest, CreateMessageResponse, Provider, ProviderError, StopReason, StreamEvent,
};
use crate::render::Renderer;
use crate::tool::ToolExecutor;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Maximum number of tool-call rounds before forcing a stop
const MAX_TURNS: usize = 30;
const MAX_RETRIES: usize = 3;
/// Estimated tokens per character (rough)
const CHARS_PER_TOKEN: usize = 4;
/// When context exceeds this fraction, inject budget pressure
const BUDGET_PRESSURE_THRESHOLD: f64 = 0.80;
/// Default context window for unknown models
const DEFAULT_CONTEXT_WINDOW: usize = 128_000;

/// Send a streaming request with retry on transient errors.
/// Ctrl+C cancels the request and returns a Cancelled error.
async fn send_with_retry(
    provider: &Arc<dyn Provider>,
    request: &CreateMessageRequest,
    renderer: &mut dyn Renderer,
) -> Result<CreateMessageResponse, ProviderError> {
    for attempt in 0..MAX_RETRIES {
        let (tx, mut rx) = mpsc::unbounded_channel::<StreamEvent>();

        let provider = Arc::clone(provider);
        let req = request.clone();
        let handle = tokio::spawn(async move { provider.create_message_stream(&req, tx).await });

        // Race: stream events vs Ctrl+C
        let mut cancelled = false;
        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Some(ev) => renderer.on_stream_event(&ev),
                        None => break, // stream finished
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    cancelled = true;
                    handle.abort();
                    break;
                }
            }
        }

        if cancelled {
            return Err(ProviderError::Stream("Cancelled by user".into()));
        }

        match handle.await {
            Ok(Ok(response)) => return Ok(response),
            Ok(Err(e)) if attempt < MAX_RETRIES - 1 && is_retryable(&e) => {
                let delay = (attempt + 1) as u64 * 2;
                warn!(
                    "Request failed (attempt {}), retrying in {delay}s: {e}",
                    attempt + 1
                );
                renderer.render_info(&format!("Connection error, retrying in {delay}s..."));
                tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
            }
            Ok(Err(e)) => return Err(e),
            Err(join_err) => {
                let msg = join_err.to_string();
                if msg.contains("cancelled") {
                    return Err(ProviderError::Stream("Cancelled by user".into()));
                }
                return Err(ProviderError::Stream(msg));
            }
        }
    }
    unreachable!()
}

fn is_retryable(err: &ProviderError) -> bool {
    let msg = err.to_string();
    match err {
        ProviderError::Http(_) | ProviderError::Stream(_) => {
            msg.contains("connection closed")
                || msg.contains("reset by peer")
                || msg.contains("timed out")
                || msg.contains("broken pipe")
                || msg.contains("EOF")
                || msg.contains("message completed")
        }
        ProviderError::RateLimit { .. } => true,
        _ => false,
    }
}

/// Run the agentic loop: model call → tool execution → model call → ...
pub async fn run_agentic_loop(
    provider: &Arc<dyn Provider>,
    model: &str,
    max_tokens: u32,
    messages: &mut Vec<Message>,
    system: Option<String>,
    executor: &ToolExecutor,
    renderer: &mut dyn Renderer,
) -> anyhow::Result<Usage> {
    let tool_defs = executor.tool_definitions();
    let mut total_usage = Usage {
        input_tokens: 0,
        output_tokens: 0,
    };
    let context_window = crate::config::model_meta::lookup_model(model)
        .map(|m| m.context_window as usize)
        .unwrap_or(DEFAULT_CONTEXT_WINDOW);

    for turn in 0..MAX_TURNS {
        info!("Agentic loop turn {turn}");
        renderer.render_thinking_start();

        // Auto-compact: if messages are too large, summarize old ones
        let estimated_tokens = estimate_message_tokens(messages);
        if estimated_tokens > context_window * 3 / 4 {
            renderer.render_info("Compacting conversation history...");
            compact_messages(messages);
        }

        // Memory level degradation based on context budget
        let usage_ratio = estimated_tokens as f64 / context_window as f64;
        let mem_level = crate::memory::MemoryLevel::from_usage(usage_ratio);

        // Rebuild system prompt with budget-aware memory + project skill detection
        let cwd = std::env::current_dir().unwrap_or_default();
        let mut effective_system = system.as_ref().map(|s| {
            let mut sys = crate::memory::inject_memory_at_level(s, &cwd, mem_level);
            // Inject auto-detected project context (only on first turn to save tokens)
            if turn == 0 {
                let profile = crate::skill_detect::detect_project(&cwd);
                let hint = crate::skill_detect::profile_to_system_hint(&profile);
                if !hint.is_empty() {
                    sys.push_str(&hint);
                }
            }
            sys
        });

        // Budget pressure: inject hint when near context limit
        if usage_ratio > BUDGET_PRESSURE_THRESHOLD {
            if let Some(ref mut sys) = effective_system {
                sys.push_str(&format!(
                    "\n\n[SYSTEM NOTE: Context usage is at {:.0}%. Memory at {}. Please wrap up concisely.]",
                    usage_ratio * 100.0, mem_level.label()
                ));
            }
        }

        // Frustration detection: check the last user message
        if let Some(last_user) = messages.iter().rev().find(|m| m.role == Role::User) {
            if let Some(ContentBlock::Text { text }) = last_user.content.first() {
                if let Some(hint) = crate::memory::detect_frustration(text) {
                    if let Some(ref mut sys) = effective_system {
                        sys.push_str(&format!("\n\n{hint}"));
                    }
                }
            }
        }

        let request = CreateMessageRequest {
            model: model.to_string(),
            messages: messages.clone(),
            system: effective_system,
            max_tokens,
            temperature: None,
            tools: tool_defs.clone(),
        };

        let response = send_with_retry(provider, &request, renderer).await?;

        // Accumulate usage
        total_usage.input_tokens += response.usage.input_tokens;
        total_usage.output_tokens += response.usage.output_tokens;

        // Add assistant message to history
        messages.push(response.message.clone());

        // Show usage
        renderer.render_status(&format!(
            "{} | {} in / {} out",
            response.model, response.usage.input_tokens, response.usage.output_tokens
        ));

        // If no tool calls, we're done — but check for premature completion claims
        if response.stop_reason != StopReason::ToolUse {
            // Completion guard: detect "I'm done" claims without evidence of actual work
            let assistant_text = response
                .message
                .content
                .iter()
                .filter_map(|b| {
                    if let ContentBlock::Text { text } = b {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            if turn > 0 && completion_guard_triggered(&assistant_text, messages) {
                renderer.render_info(
                    "⚠ Completion claim detected without file changes. Asking model to verify...",
                );
                // Inject a nudge to actually do the work
                messages.push(Message {
                    role: Role::User,
                    content: vec![ContentBlock::Text {
                        text: "[SYSTEM: You claimed the task is complete, but no file modifications were detected in this session. Please either (1) actually perform the changes using tools, or (2) explain specifically why no changes are needed.]".to_string(),
                    }],
                });
                continue; // re-enter the loop
            }

            // Hermes: analyze interaction to learn preferences
            let cwd = std::env::current_dir().unwrap_or_default();
            let user_text = messages
                .iter()
                .rev()
                .find(|m| m.role == Role::User)
                .and_then(|m| m.content.first())
                .and_then(|b| {
                    if let ContentBlock::Text { text } = b {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .unwrap_or("");
            crate::memory::hermes_analyze(user_text, &assistant_text, &cwd);

            return Ok(total_usage);
        }

        // Execute tool calls
        let tool_uses: Vec<(String, String, serde_json::Value)> = response
            .message
            .content
            .iter()
            .filter_map(|block| {
                if let ContentBlock::ToolUse { id, name, input } = block {
                    Some((id.clone(), name.clone(), input.clone()))
                } else {
                    None
                }
            })
            .collect();

        if tool_uses.is_empty() {
            return Ok(total_usage);
        }

        let mut tool_results = Vec::new();
        for (id, name, input) in &tool_uses {
            renderer.render_tool_start(name, id);
            let output = executor.execute(name, input).await;

            // Error file pre-injection: if bash fails, extract file paths from stderr
            let mut final_content = output.content.clone();
            if output.is_error && name == "bash" {
                let injected = extract_and_read_error_files(&output.content, &executor).await;
                if !injected.is_empty() {
                    final_content.push_str("\n\n--- Referenced files from error ---\n");
                    final_content.push_str(&injected);
                }
            }

            // Truncate very large tool results
            if final_content.len() > 30000 {
                final_content.truncate(30000);
                final_content.push_str("\n... (output pruned to save context)");
            }

            renderer.render_tool_result(name, &final_content, output.is_error);

            tool_results.push(ContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: final_content,
                is_error: output.is_error,
            });
        }

        // Add tool results as a user message
        messages.push(Message {
            role: Role::User,
            content: tool_results,
        });
    }

    renderer.render_error(&format!("Reached maximum turns ({MAX_TURNS}), stopping."));
    Ok(total_usage)
}

/// Estimate total tokens in message history
fn estimate_message_tokens(messages: &[Message]) -> usize {
    let total_chars: usize = messages
        .iter()
        .map(|m| {
            m.content
                .iter()
                .map(|b| match b {
                    ContentBlock::Text { text } => text.len(),
                    ContentBlock::ToolUse { input, .. } => input.to_string().len(),
                    ContentBlock::ToolResult { content, .. } => content.len(),
                    // Rough heuristic: base64 image ~= data bytes * 4/3. Don't blow
                    // up the token budget with the actual byte count — treat each
                    // image as ~1000 tokens for estimation purposes.
                    ContentBlock::Image { .. } => 1000 * crate::query::CHARS_PER_TOKEN,
                })
                .sum::<usize>()
        })
        .sum();
    total_chars / CHARS_PER_TOKEN
}

/// Public wrapper for manual /compact command
pub fn compact_messages_public(messages: &mut Vec<Message>) {
    compact_messages(messages);
}

/// Compact old messages: keep system context, summarize middle, keep recent
fn compact_messages(messages: &mut Vec<Message>) {
    if messages.len() <= 4 {
        return;
    }

    // Keep first 2 and last 4 messages, summarize the rest
    let keep_start = 2;
    let keep_end = 4;
    let total = messages.len();

    if total <= keep_start + keep_end {
        return;
    }

    let middle_count = total - keep_start - keep_end;
    let summary = format!(
        "[{middle_count} earlier messages compacted to save context. The conversation started with the user's initial request and involved tool calls and responses.]"
    );

    let mut new_messages = Vec::new();
    new_messages.extend_from_slice(&messages[..keep_start]);
    new_messages.push(Message {
        role: Role::User,
        content: vec![ContentBlock::text(summary)],
    });
    new_messages.extend_from_slice(&messages[total - keep_end..]);
    *messages = new_messages;
}

/// Extract file paths from error output and read them
async fn extract_and_read_error_files(error_output: &str, _executor: &ToolExecutor) -> String {
    let mut result = String::new();
    // Common patterns: "file.rs:123:", "Error in ./src/main.rs", etc.
    let re = regex::Regex::new(r"(?:^|[\s:])([./]?[a-zA-Z_][\w./\-]*\.[a-zA-Z]{1,10}):(\d+)").ok();
    if let Some(re) = re {
        let mut seen = std::collections::HashSet::new();
        for cap in re.captures_iter(error_output) {
            let path = cap[1].to_string();
            if seen.contains(&path) {
                continue;
            }
            seen.insert(path.clone());

            if seen.len() > 3 {
                break;
            } // max 3 files

            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                let line_num: usize = cap[2].parse().unwrap_or(1);
                let lines: Vec<&str> = content.lines().collect();
                let start = line_num.saturating_sub(5);
                let end = (line_num + 5).min(lines.len());
                result.push_str(&format!("\n--- {path}:{start}-{end} ---\n"));
                for (i, line) in lines[start..end].iter().enumerate() {
                    let n = start + i + 1;
                    let marker = if n == line_num { ">>>" } else { "   " };
                    result.push_str(&format!("{marker} {n:>5} | {line}\n"));
                }
            }
        }
    }
    result
}

/// Attempt to repair broken JSON from weak models
pub fn repair_json(raw: &str) -> Option<serde_json::Value> {
    // Try direct parse first
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) {
        return Some(v);
    }

    let mut s = raw.trim().to_string();

    // Strip markdown code fences
    if s.starts_with("```json") {
        s = s.strip_prefix("```json").unwrap_or(&s).to_string();
    } else if s.starts_with("```") {
        s = s.strip_prefix("```").unwrap_or(&s).to_string();
    }
    if s.ends_with("```") {
        s = s.strip_suffix("```").unwrap_or(&s).to_string();
    }
    s = s.trim().to_string();

    // Remove trailing commas before } or ]
    let re_comma = regex::Regex::new(r",\s*([}\]])").ok()?;
    s = re_comma.replace_all(&s, "$1").to_string();

    // Try to fix unbalanced brackets
    let open_braces = s.chars().filter(|c| *c == '{').count();
    let close_braces = s.chars().filter(|c| *c == '}').count();
    for _ in 0..(open_braces.saturating_sub(close_braces)) {
        s.push('}');
    }
    let open_brackets = s.chars().filter(|c| *c == '[').count();
    let close_brackets = s.chars().filter(|c| *c == ']').count();
    for _ in 0..(open_brackets.saturating_sub(close_brackets)) {
        s.push(']');
    }

    serde_json::from_str::<serde_json::Value>(&s).ok()
}

// ── Completion Guard ──
// Detects when the AI claims "done" but hasn't actually modified any files.
// Only triggers once per loop (via the `turn > 0` check in the caller).

static COMPLETION_GUARD_FIRED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn completion_guard_triggered(assistant_text: &str, messages: &[Message]) -> bool {
    // Only fire once per session turn
    if COMPLETION_GUARD_FIRED.load(std::sync::atomic::Ordering::Relaxed) {
        return false;
    }

    let lower = assistant_text.to_lowercase();

    // Check if the assistant claims completion
    let completion_phrases = [
        "任务已完成",
        "任务已全部完成",
        "全部完成",
        "已经完成",
        "所有修改已完成",
        "已全部搞定",
        "大功告成",
        "圆满完成",
        "工作已完成",
        "实施完成",
        "all done",
        "task complete",
        "implementation complete",
        "all set",
        "fully complete",
        "mission accomplished",
        "changes are done",
        "i've completed",
        "i have completed",
        "everything is done",
    ];
    let claims_done = completion_phrases.iter().any(|p| lower.contains(p));
    if !claims_done {
        return false;
    }

    // Check if any file_write/file_edit/multi_edit tool was actually used
    let write_tools = [
        "file_edit",
        "file_write",
        "multi_edit",
        "file_append",
        "parallel_edit",
    ];
    let has_file_change = messages.iter().any(|m| {
        m.content.iter().any(|b| {
            if let ContentBlock::ToolUse { name, .. } = b {
                write_tools.contains(&name.as_str())
            } else {
                false
            }
        })
    });

    if has_file_change {
        return false; // legit completion
    }

    // No file changes but claims done → suspicious
    COMPLETION_GUARD_FIRED.store(true, std::sync::atomic::Ordering::Relaxed);
    true
}
