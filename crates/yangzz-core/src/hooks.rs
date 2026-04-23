use serde_json::Value;
use std::path::Path;
use tokio::process::Command;
use tracing::{info, warn};

/// Hook events that can trigger user-defined scripts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    /// Before a tool is executed
    PreTool,
    /// After a tool is executed
    PostTool,
    /// When a session starts
    SessionStart,
    /// When a session ends
    SessionEnd,
    /// Before sending a message to the model
    PreMessage,
    /// After receiving a response from the model
    PostMessage,
}

impl HookEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            HookEvent::PreTool => "pre_tool",
            HookEvent::PostTool => "post_tool",
            HookEvent::SessionStart => "session_start",
            HookEvent::SessionEnd => "session_end",
            HookEvent::PreMessage => "pre_message",
            HookEvent::PostMessage => "post_message",
        }
    }
}

/// Hook configuration loaded from .yangzz/hooks/ directory
#[derive(Debug)]
pub struct HookConfig {
    pub event: HookEvent,
    pub script: String,
}

/// Load hooks from project directory (.yangzz/hooks/)
pub fn load_hooks(cwd: &Path) -> Vec<HookConfig> {
    let hooks_dir = cwd.join(".yangzz").join("hooks");
    if !hooks_dir.exists() {
        return Vec::new();
    }

    let mut hooks = Vec::new();
    let events = [
        HookEvent::PreTool,
        HookEvent::PostTool,
        HookEvent::SessionStart,
        HookEvent::SessionEnd,
        HookEvent::PreMessage,
        HookEvent::PostMessage,
    ];

    for event in &events {
        let script_path = hooks_dir.join(event.as_str());
        if script_path.exists() {
            if let Ok(script) = std::fs::read_to_string(&script_path) {
                hooks.push(HookConfig {
                    event: *event,
                    script,
                });
                info!("Loaded hook: {}", event.as_str());
            }
        }
        // Also check .sh extension
        let script_path_sh = hooks_dir.join(format!("{}.sh", event.as_str()));
        if script_path_sh.exists() {
            if let Ok(script) = std::fs::read_to_string(&script_path_sh) {
                hooks.push(HookConfig {
                    event: *event,
                    script,
                });
                info!("Loaded hook: {}.sh", event.as_str());
            }
        }
    }

    hooks
}

/// Run all hooks matching an event
pub async fn run_hooks(
    hooks: &[HookConfig],
    event: HookEvent,
    env_vars: &[(&str, &str)],
    cwd: &Path,
) {
    for hook in hooks.iter().filter(|h| h.event == event) {
        info!("Running hook: {}", event.as_str());
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&hook.script).current_dir(cwd);

        for (key, val) in env_vars {
            cmd.env(key, val);
        }

        match cmd.output().await {
            Ok(output) => {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!("Hook {} failed: {}", event.as_str(), stderr.trim());
                }
            }
            Err(e) => {
                warn!("Hook {} error: {e}", event.as_str());
            }
        }
    }
}

/// Run hooks with tool-specific environment variables
pub async fn run_tool_hooks(
    hooks: &[HookConfig],
    event: HookEvent,
    tool_name: &str,
    tool_input: &Value,
    cwd: &Path,
) {
    let input_str = tool_input.to_string();
    let vars: Vec<(&str, &str)> = vec![
        ("YANGZZ_TOOL", tool_name),
        ("YANGZZ_TOOL_INPUT", &input_str),
    ];
    run_hooks(hooks, event, &vars, cwd).await;
}
