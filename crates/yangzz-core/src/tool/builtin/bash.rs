use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Command;

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str { "bash" }

    fn description(&self) -> &str {
        "Execute a bash command in the shell. Use for running scripts, installing packages, or any shell operation."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 30)",
                    "default": 30
                }
            },
            "required": ["command"]
        })
    }

    fn is_destructive(&self) -> bool { true }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let command = input["command"]
            .as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'command' field".into()))?;

        let timeout_secs = input["timeout_secs"].as_u64().unwrap_or(30);

        // Load execution policy and apply sandbox/command restrictions
        let policy = crate::sandbox::load_policy(&ctx.cwd);

        // Check command policy
        if let Err(e) = crate::sandbox::check_command_policy(command, &policy.commands) {
            return Err(ToolError::Validation(format!("Policy denied: {e}")));
        }

        // Apply sandbox wrapping if enabled
        let sandbox_mode = crate::sandbox::SandboxMode::from_str(&policy.sandbox.mode);
        let effective_command = crate::sandbox::wrap_command(
            command, &ctx.cwd, sandbox_mode, &policy.network
        );

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs.min(policy.commands.max_runtime_secs)),
            if cfg!(target_os = "windows") {
                Command::new("cmd")
                    .arg("/C")
                    .arg(&effective_command)
                    .current_dir(&ctx.cwd)
                    .output()
            } else {
                Command::new("bash")
                    .arg("-c")
                    .arg(&effective_command)
                    .current_dir(&ctx.cwd)
                    .output()
            }
        ).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let exit_code = output.status.code().unwrap_or(-1);

                let mut result = String::new();
                if !stdout.is_empty() {
                    result.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !result.is_empty() { result.push('\n'); }
                    result.push_str("STDERR:\n");
                    result.push_str(&stderr);
                }
                if result.is_empty() {
                    result = format!("(no output, exit code: {exit_code})");
                } else {
                    result.push_str(&format!("\n(exit code: {exit_code})"));
                }

                // Truncate very long output
                if result.len() > 50000 {
                    result.truncate(50000);
                    result.push_str("\n... (output truncated)");
                }

                if output.status.success() {
                    Ok(ToolOutput::success(result))
                } else {
                    Ok(ToolOutput::error(result))
                }
            }
            Ok(Err(e)) => Err(ToolError::Execution(format!("Failed to run command: {e}"))),
            Err(_) => Err(ToolError::Execution(format!("Command timed out after {timeout_secs}s"))),
        }
    }
}
