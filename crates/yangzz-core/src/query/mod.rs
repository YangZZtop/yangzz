use crate::message::{ContentBlock, Message, Role, Usage};
use crate::provider::{
    CreateMessageRequest, CreateMessageResponse, Provider, ProviderError, StopReason, StreamEvent,
};
use crate::render::Renderer;
use crate::tool::ToolExecutor;
use std::collections::VecDeque;
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

/// Consecutive identical tool calls before triggering loop detection
const LOOP_DETECTION_THRESHOLD: usize = 3;

/// Tracks recent tool calls to detect repetitive loops.
struct LoopDetector {
    /// Ring buffer of recent (tool_name, args_hash) pairs
    recent: VecDeque<(String, u64)>,
}

impl LoopDetector {
    fn new() -> Self {
        Self {
            recent: VecDeque::with_capacity(LOOP_DETECTION_THRESHOLD + 1),
        }
    }

    /// Record a tool call. Returns true if a loop is detected.
    fn record(&mut self, tool_name: &str, args: &serde_json::Value) -> bool {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        args.to_string().hash(&mut hasher);
        let hash = hasher.finish();

        let entry = (tool_name.to_string(), hash);

        if self.recent.len() >= LOOP_DETECTION_THRESHOLD {
            self.recent.pop_front();
        }
        self.recent.push_back(entry);

        if self.recent.len() < LOOP_DETECTION_THRESHOLD {
            return false;
        }

        // Check if last N calls are identical
        let last = &self.recent[self.recent.len() - 1];
        self.recent.iter().rev().take(LOOP_DETECTION_THRESHOLD).all(|e| e == last)
    }

    fn reset(&mut self) {
        self.recent.clear();
    }
}

/// Maximum time to wait for the first token from the provider before giving up.
const STREAM_FIRST_TOKEN_TIMEOUT_SECS: u64 = 120;
/// Maximum time to wait between consecutive stream events (stall detection).
const STREAM_STALL_TIMEOUT_SECS: u64 = 60;

/// Send a streaming request with retry on transient errors.
/// Ctrl+C cancels the request and returns a Cancelled error.
/// Includes timeout protection against provider hangs.
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

        // Race: stream events vs Ctrl+C vs timeout
        let mut cancelled = false;
        let mut timed_out = false;
        let mut got_first_token = false;
        loop {
            let timeout_duration = if got_first_token {
                std::time::Duration::from_secs(STREAM_STALL_TIMEOUT_SECS)
            } else {
                std::time::Duration::from_secs(STREAM_FIRST_TOKEN_TIMEOUT_SECS)
            };

            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Some(ev) => {
                            got_first_token = true;
                            renderer.on_stream_event(&ev);
                        }
                        None => break, // stream finished
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    cancelled = true;
                    handle.abort();
                    break;
                }
                _ = tokio::time::sleep(timeout_duration) => {
                    // Stream stalled — provider is not responding
                    timed_out = true;
                    handle.abort();
                    break;
                }
            }
        }

        if cancelled {
            return Err(ProviderError::Stream("Cancelled by user".into()));
        }

        if timed_out {
            renderer.stop_spinner();
            let timeout_kind = if got_first_token {
                "Stream stalled (no data for 60s)"
            } else {
                "No response from provider (120s timeout)"
            };
            if attempt < MAX_RETRIES - 1 {
                let delay = (attempt + 1) as u64 * 3;
                warn!("{timeout_kind}, retrying in {delay}s (attempt {})", attempt + 1);
                renderer.render_info(&format!(
                    "⏱ {timeout_kind} — retry {}/{} in {delay}s",
                    attempt + 1,
                    MAX_RETRIES - 1,
                ));
                tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                renderer.render_thinking_start();
                continue;
            } else {
                return Err(ProviderError::Stream(format!(
                    "{timeout_kind}. Provider may be down or overloaded."
                )));
            }
        }

        match handle.await {
            Ok(Ok(response)) => return Ok(response),
            Ok(Err(e)) if attempt < MAX_RETRIES - 1 && is_retryable(&e) => {
                let delay = (attempt + 1) as u64 * 2;
                warn!(
                    "Request failed (attempt {}), retrying in {delay}s: {e}",
                    attempt + 1
                );
                renderer.render_info(&format!(
                    "Retry {}/{} in {delay}s — {}",
                    attempt + 1,
                    MAX_RETRIES - 1,
                    short_error(&e)
                ));
                tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                renderer.render_thinking_start();
            }
            Ok(Err(e)) => return Err(e),
            Err(join_err) => {
                let msg = join_err.to_string();
                if msg.contains("cancelled") {
                    return Err(ProviderError::Stream("Cancelled by user".into()));
                }
                // Task was aborted (timeout case already handled above)
                if msg.contains("aborted") || msg.contains("JoinError") {
                    if attempt < MAX_RETRIES - 1 {
                        continue;
                    }
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
        ProviderError::Api { status, .. } => *status >= 500,
        _ => false,
    }
}

fn short_error(err: &ProviderError) -> &'static str {
    match err {
        ProviderError::RateLimit { .. } => "rate limited",
        ProviderError::Http(_) => "network error",
        ProviderError::Api { status, .. } if *status >= 500 => "server error",
        ProviderError::Stream(_) => "stream interrupted",
        _ => "transient error",
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
    run_agentic_loop_bounded(provider, model, max_tokens, messages, system, executor, renderer, MAX_TURNS).await
}

/// Run the agentic loop with a custom turn limit (used by sub-agents)
pub async fn run_agentic_loop_bounded(
    provider: &Arc<dyn Provider>,
    model: &str,
    max_tokens: u32,
    messages: &mut Vec<Message>,
    system: Option<String>,
    executor: &ToolExecutor,
    renderer: &mut dyn Renderer,
    max_turns: usize,
) -> anyhow::Result<Usage> {
    let tool_defs = executor.tool_definitions();
    let mut total_usage = Usage {
        input_tokens: 0,
        output_tokens: 0,
    };
    let mut loop_detector = LoopDetector::new();
    let mut tool_cache = ToolCache::new();
    let context_window = crate::config::model_meta::lookup_model(model)
        .map(|m| m.context_window as usize)
        .unwrap_or(DEFAULT_CONTEXT_WINDOW);

    // Reset completion guard for this new agentic loop invocation
    COMPLETION_GUARD_FIRED.store(false, std::sync::atomic::Ordering::Relaxed);

    for turn in 0..max_turns {
        info!("Agentic loop turn {turn}");
        renderer.render_thinking_start();

        // Auto-compact: if messages are too large, summarize old ones
        let estimated_tokens = estimate_message_tokens(messages);
        if estimated_tokens > context_window * 3 / 4 {
            renderer.render_info("Compacting conversation history...");
            compact_messages_with_summary(messages, provider, model).await;
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

        // Pre-send cost estimate (show on first turn only to avoid noise)
        if turn == 0 {
            let est_tokens = estimate_message_tokens(messages);
            if let Some(meta) = crate::config::model_meta::lookup_model(model) {
                let est_cost = est_tokens as f64 / 1_000_000.0 * meta.input_price;
                if est_cost > 0.005 {
                    renderer.render_status(&format!(
                        "~{} input tokens · ~${:.4} estimated",
                        est_tokens, est_cost
                    ));
                }
            }
        }

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

        // ── Parallel execution for read-only tools ──
        // Split tool calls into read-only (parallelizable) and write (serial).
        // If ALL are read-only, run them all in parallel. If mixed, run serial
        // to preserve ordering semantics (a write might depend on a prior read).
        let all_read_only = tool_uses.iter().all(|(_, name, _)| is_read_only_tool(name));

        let mut tool_results = Vec::new();
        let mut loop_blocked = false;

        if all_read_only && tool_uses.len() > 1 {
            // Parallel execution path — execute read-only tools with batched rendering
            // First check loop detection for all
            for (id, name, input) in &tool_uses {
                if loop_detector.record(name, input) {
                    warn!("Loop detected: {name} called {LOOP_DETECTION_THRESHOLD} times with same args");
                    renderer.render_error(&format!(
                        "Loop detected: `{name}` called {} times with identical arguments.",
                        LOOP_DETECTION_THRESHOLD
                    ));
                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: format!(
                            "[LOOP DETECTED] `{name}` called {} times with same args. Try a different approach.",
                            LOOP_DETECTION_THRESHOLD
                        ),
                        is_error: true,
                    });
                    loop_blocked = true;
                    break;
                }
            }

            if !loop_blocked {
                // Show all tools starting at once (batched visual)
                for (id, name, input) in &tool_uses {
                    renderer.render_tool_start_with_input(name, id, input);
                    renderer.stop_spinner();
                }

                // Execute all read-only tools (sequential but fast — no permission prompts)
                let mut results = Vec::new();
                for (_id, name, input) in &tool_uses {
                    let output = if let Some(cached) = tool_cache.get(name, input) {
                        crate::tool::ToolOutput::success(cached.clone())
                    } else {
                        let result = executor.execute(name, input).await;
                        if !result.is_error {
                            tool_cache.put(name, input, &result.content);
                        }
                        result
                    };
                    results.push(output);
                }

                // Render all results
                for ((id, name, _input), output) in tool_uses.iter().zip(results.iter()) {
                    let mut final_content = output.content.clone();
                    if final_content.len() > 16000 {
                        let head = &final_content[..6000];
                        let tail_start = final_content.len().saturating_sub(2000);
                        let tail = &final_content[tail_start..];
                        let omitted = final_content.len() - 8000;
                        final_content = format!(
                            "{head}\n\n... [{omitted} chars omitted] ...\n\n{tail}\n(output pruned)"
                        );
                    }
                    renderer.render_tool_result(name, &final_content, output.is_error);
                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: final_content,
                        is_error: output.is_error,
                    });
                }
            }
        } else {
            // Serial execution path (original behavior)
            for (id, name, input) in &tool_uses {
                if loop_detector.record(name, input) {
                    warn!("Loop detected: {name} called {LOOP_DETECTION_THRESHOLD} times with same args");
                    renderer.render_error(&format!(
                        "Loop detected: `{name}` called {} times with identical arguments. Blocking and asking model to try a different approach.",
                        LOOP_DETECTION_THRESHOLD
                    ));
                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: format!(
                            "[LOOP DETECTED] You have called `{name}` {} consecutive times with the same arguments and gotten the same result. This approach is not working. Please try a fundamentally different strategy.",
                            LOOP_DETECTION_THRESHOLD
                        ),
                        is_error: true,
                    });
                    loop_blocked = true;
                    break;
                }

                renderer.render_tool_start_with_input(name, id, input);
                let output = if let Some(cached) = tool_cache.get(name, input) {
                    // Cache hit — skip execution
                    crate::tool::ToolOutput::success(cached.clone())
                } else {
                    let result = executor.execute(name, input).await;
                    // Cache read-only results; invalidate on writes
                    if is_read_only_tool(name) && !result.is_error {
                        tool_cache.put(name, input, &result.content);
                    } else if !is_read_only_tool(name) {
                        tool_cache.invalidate_after_write();
                    }
                    result
                };

                // Error file pre-injection: if bash fails, extract file paths from stderr
                let mut final_content = output.content.clone();
                if output.is_error && name == "bash" {
                    let injected = extract_and_read_error_files(&output.content, &executor).await;
                    if !injected.is_empty() {
                        final_content.push_str("\n\n--- Referenced files from error ---\n");
                        final_content.push_str(&injected);
                    }
                }

                // Truncate large tool results: keep head + tail for context
                if final_content.len() > 16000 {
                    let head = &final_content[..6000];
                    let tail_start = final_content.len().saturating_sub(2000);
                    let tail = &final_content[tail_start..];
                    let omitted = final_content.len() - 8000;
                    final_content = format!(
                        "{head}\n\n... [{omitted} chars omitted] ...\n\n{tail}\n(output pruned: kept first 6K + last 2K)"
                    );
                }

                renderer.render_tool_result(name, &final_content, output.is_error);

                tool_results.push(ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content: final_content,
                    is_error: output.is_error,
                });
            }
        } // end parallel vs serial

        // If loop was blocked, add error results for remaining tool calls
        if loop_blocked {
            for (id, _name, _input) in tool_uses.iter().skip(tool_results.len()) {
                tool_results.push(ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content: "[SKIPPED] Previous tool call was blocked due to loop detection.".to_string(),
                    is_error: true,
                });
            }
            loop_detector.reset();
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

/// Check if a tool is read-only (safe for parallel execution)
fn is_read_only_tool(name: &str) -> bool {
    matches!(
        name,
        "file_read"
            | "grep"
            | "glob"
            | "list_dir"
            | "tree"
            | "web_search"
            | "browser"
            | "fetch"
            | "code_graph"
            | "notebook_read"
    )
}

/// Simple tool result cache — avoids re-reading the same file multiple times
/// within a single agentic loop invocation.
struct ToolCache {
    entries: std::collections::HashMap<u64, String>,
}

impl ToolCache {
    fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    fn cache_key(name: &str, input: &serde_json::Value) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        name.hash(&mut hasher);
        input.to_string().hash(&mut hasher);
        hasher.finish()
    }

    fn get(&self, name: &str, input: &serde_json::Value) -> Option<&String> {
        if !is_read_only_tool(name) {
            return None;
        }
        let key = Self::cache_key(name, input);
        self.entries.get(&key)
    }

    fn put(&mut self, name: &str, input: &serde_json::Value, result: &str) {
        if !is_read_only_tool(name) {
            return;
        }
        // Don't cache very large results (> 32KB) — not worth the memory
        if result.len() > 32_000 {
            return;
        }
        let key = Self::cache_key(name, input);
        self.entries.insert(key, result.to_string());
    }

    /// Invalidate cache entries that might be stale after a write operation.
    /// Called after any file_write/file_edit/bash execution.
    fn invalidate_after_write(&mut self) {
        // Conservative: clear all file_read entries since we don't track
        // which specific file was modified.
        self.entries.clear();
    }
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
                    ContentBlock::Thinking { text } => text.len(),
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

/// Public wrapper for manual /compact command (sync fallback)
pub fn compact_messages_public(messages: &mut Vec<Message>) {
    compact_messages_truncate(messages);
}

/// Async compact: summarize middle messages using the model
async fn compact_messages_with_summary(
    messages: &mut Vec<Message>,
    provider: &Arc<dyn Provider>,
    model: &str,
) {
    if messages.len() <= 6 {
        return;
    }

    let keep_start = 2;
    let keep_end = 6;
    let total = messages.len();

    if total <= keep_start + keep_end {
        return;
    }

    let middle = &messages[keep_start..total - keep_end];
    let middle_text = summarize_messages_to_text(middle);

    // Ask the model to summarize
    let summary_prompt = format!(
        "Summarize the following conversation excerpt in 2-4 concise paragraphs. \
         Focus on: key decisions made, files modified, tool results, and any unresolved issues. \
         Do NOT include greetings or filler. Output ONLY the summary.\n\n---\n{middle_text}"
    );

    let request = CreateMessageRequest {
        model: model.to_string(),
        messages: vec![Message::user(&summary_prompt)],
        system: None,
        max_tokens: 1024,
        temperature: Some(0.0),
        tools: vec![],
    };

    let summary = match provider.create_message(&request).await {
        Ok(resp) => resp
            .message
            .content
            .iter()
            .filter_map(|b| b.as_text())
            .collect::<Vec<_>>()
            .join(""),
        Err(_) => {
            // Fallback to truncation if summarization fails
            compact_messages_truncate(messages);
            return;
        }
    };

    let middle_count = total - keep_start - keep_end;
    let compact_text = format!(
        "[{middle_count} earlier messages compacted. Summary:\n{summary}]"
    );

    let mut new_messages = Vec::new();
    new_messages.extend_from_slice(&messages[..keep_start]);
    new_messages.push(Message {
        role: Role::User,
        content: vec![ContentBlock::text(compact_text)],
    });
    new_messages.extend_from_slice(&messages[total - keep_end..]);
    *messages = new_messages;
}

/// Fallback: truncate without LLM summarization
fn compact_messages_truncate(messages: &mut Vec<Message>) {
    if messages.len() <= 6 {
        return;
    }

    let keep_start = 2;
    let keep_end = 6;
    let total = messages.len();

    if total <= keep_start + keep_end {
        return;
    }

    let middle_count = total - keep_start - keep_end;
    let summary = format!(
        "[{middle_count} earlier messages compacted to save context. The conversation covered tool calls, file edits, and iterative problem-solving.]"
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

/// Extract readable text from a slice of messages for summarization
fn summarize_messages_to_text(messages: &[Message]) -> String {
    let mut out = String::new();
    for msg in messages {
        let role = match msg.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
            Role::System => "System",
        };
        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => {
                    // Truncate very long text blocks
                    let truncated = if text.len() > 500 {
                        format!("{}...[truncated]", &text[..500])
                    } else {
                        text.clone()
                    };
                    out.push_str(&format!("{role}: {truncated}\n"));
                }
                ContentBlock::ToolUse { name, .. } => {
                    out.push_str(&format!("{role}: [called tool: {name}]\n"));
                }
                ContentBlock::ToolResult { content, is_error, .. } => {
                    let status = if *is_error { "error" } else { "ok" };
                    let truncated = if content.len() > 200 {
                        format!("{}...", &content[..200])
                    } else {
                        content.clone()
                    };
                    out.push_str(&format!("{role}: [tool result ({status}): {truncated}]\n"));
                }
                ContentBlock::Image { .. } => {
                    out.push_str(&format!("{role}: [image]\n"));
                }
                ContentBlock::Thinking { .. } => {
                    out.push_str(&format!("{role}: [thinking]\n"));
                }
            }
        }
    }
    // Cap total size to avoid blowing up the summarization request
    if out.len() > 8000 {
        out.truncate(8000);
        out.push_str("\n...[truncated for summarization]");
    }
    out
}

/// Extract file paths from error output and read them
async fn extract_and_read_error_files(error_output: &str, _executor: &ToolExecutor) -> String {
    let mut result = String::new();
    let mut seen = std::collections::HashSet::new();

    // Pattern 1: "file.ext:line:" (Rust, GCC, TypeScript, ESLint)
    // Pattern 2: 'File "path", line N' (Python traceback)
    // Pattern 3: "at path:line:col" (Node.js stack trace)
    let patterns = [
        regex::Regex::new(r#"(?:^|[\s:])([./]?[a-zA-Z_][\w./\-]*\.[a-zA-Z]{1,10}):(\d+)"#).ok(),
        regex::Regex::new(r#"File "([^"]+)", line (\d+)"#).ok(),
        regex::Regex::new(r#"at\s+(?:\S+\s+\()?([./][^\s:]+):(\d+)"#).ok(),
    ];

    for re in patterns.iter().flatten() {
        for cap in re.captures_iter(error_output) {
            let path = cap[1].to_string();
            if seen.contains(&path) || seen.len() >= 3 {
                continue;
            }
            seen.insert(path.clone());

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
