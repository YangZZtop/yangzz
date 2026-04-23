use super::{ToolContext, ToolOutput, ToolRegistry};
use crate::hooks::{self, HookConfig, HookEvent};
use crate::permission::{self, PermissionManager};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Undo entry: stores old file content before modification
#[derive(Debug, Clone)]
pub struct UndoEntry {
    pub path: String,
    pub old_content: String,
    pub tool_name: String,
}

/// Tool execution pipeline: lookup → permission → execute
pub struct ToolExecutor {
    registry: ToolRegistry,
    permission: Arc<PermissionManager>,
    ctx: ToolContext,
    /// Recent tool call log for loop detection: (tool_name, input_hash)
    recent_calls: Mutex<VecDeque<(String, u64)>>,
    /// Undo stack (max 20 entries)
    undo_stack: Mutex<VecDeque<UndoEntry>>,
    /// Lifecycle hooks
    hooks: Vec<HookConfig>,
}

impl ToolExecutor {
    pub fn new(registry: ToolRegistry, permission: Arc<PermissionManager>, cwd: std::path::PathBuf) -> Self {
        let hook_list = hooks::load_hooks(&cwd);
        Self {
            registry,
            permission,
            ctx: ToolContext { cwd },
            recent_calls: Mutex::new(VecDeque::new()),
            undo_stack: Mutex::new(VecDeque::new()),
            hooks: hook_list,
        }
    }

    /// Execute a tool call from the model
    pub async fn execute(&self, name: &str, input: &serde_json::Value) -> ToolOutput {
        // 1. Lookup
        let tool = match self.registry.get(name) {
            Some(t) => t,
            None => {
                warn!("Tool not found: {name}");
                return ToolOutput::error(format!("Tool '{name}' not found"));
            }
        };

        // 2. Tool loop detection: same (name, input) 3+ times → block
        {
            let input_hash = hash_value(input);
            let mut calls = self.recent_calls.lock().await;
            calls.push_back((name.to_string(), input_hash));
            if calls.len() > 20 { calls.pop_front(); }
            let repeat_count = calls.iter().filter(|(n, h)| n == name && *h == input_hash).count();
            if repeat_count >= 3 {
                warn!("Tool loop detected: {name} called {repeat_count} times with same input");
                return ToolOutput::error(format!(
                    "Tool loop detected: '{name}' called {repeat_count} times with identical arguments. Breaking loop."
                ));
            }
        }

        // 3. Permission check
        if !tool.is_read_only() {
            match self.permission.check(name, input, tool.is_destructive()).await {
                Ok(true) => {} // allowed
                Ok(false) => {
                    info!("Tool {name} denied by user");
                    return ToolOutput::error(format!("Permission denied for tool '{name}'"));
                }
                Err(e) => {
                    return ToolOutput::error(format!("Permission error: {e}"));
                }
            }
        }

        // 4. Pre-execute: save undo state for write tools
        if matches!(name, "file_edit" | "file_write" | "multi_edit" | "file_append" | "notebook_edit") {
            if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                let full_path = if std::path::Path::new(path).is_absolute() {
                    std::path::PathBuf::from(path)
                } else {
                    self.ctx.cwd.join(path)
                };
                if let Ok(old_content) = tokio::fs::read_to_string(&full_path).await {
                    let mut stack = self.undo_stack.lock().await;
                    stack.push_back(UndoEntry {
                        path: full_path.to_string_lossy().to_string(),
                        old_content,
                        tool_name: name.to_string(),
                    });
                    if stack.len() > 20 { stack.pop_front(); }
                }
            }
        }

        // 5. Pre-tool hook
        hooks::run_tool_hooks(&self.hooks, HookEvent::PreTool, name, input, &self.ctx.cwd).await;

        // 6. Execute
        info!("Executing tool: {name}");
        let output = match tool.execute(input, &self.ctx).await {
            Ok(output) => output,
            Err(e) => {
                warn!("Tool {name} failed: {e}");
                ToolOutput::error(format!("Tool execution failed: {e}"))
            }
        };

        // 7. Post-tool hook
        hooks::run_tool_hooks(&self.hooks, HookEvent::PostTool, name, input, &self.ctx.cwd).await;

        // 8. Post-execute: secret scan on output
        let secret_warnings = permission::scan_for_secrets(&output.content);
        if !secret_warnings.is_empty() {
            warn!("Secret detected in tool output: {:?}", secret_warnings);
            let mut redacted = output.content.clone();
            redacted.push_str("\n\n⚠️ WARNING: ");
            for w in &secret_warnings {
                redacted.push_str(w);
                redacted.push('\n');
            }
            return ToolOutput { content: redacted, is_error: output.is_error };
        }

        output
    }

    /// Undo the last file modification
    pub async fn undo(&self) -> Option<String> {
        let mut stack = self.undo_stack.lock().await;
        if let Some(entry) = stack.pop_back() {
            match std::fs::write(&entry.path, &entry.old_content) {
                Ok(()) => Some(format!("Undid {} on {}", entry.tool_name, entry.path)),
                Err(e) => Some(format!("Undo failed: {e}")),
            }
        } else {
            Some("Nothing to undo".to_string())
        }
    }

    pub fn tool_definitions(&self) -> Vec<crate::provider::ToolDefinition> {
        self.registry.tool_definitions()
    }
}

/// Simple hash for loop detection
fn hash_value(v: &serde_json::Value) -> u64 {
    use std::hash::{Hash, Hasher};
    let s = v.to_string();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}
