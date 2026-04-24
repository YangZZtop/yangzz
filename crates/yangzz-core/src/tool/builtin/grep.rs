use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::process::Command;

pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for a pattern in files using ripgrep (rg) or grep. Returns matching lines with file paths and line numbers."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Search pattern (regex by default)"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search in (default: current directory)"
                },
                "include": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g. '*.rs')"
                },
                "fixed_strings": {
                    "type": "boolean",
                    "description": "Treat pattern as literal string instead of regex"
                }
            },
            "required": ["pattern"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let pattern = input["pattern"]
            .as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'pattern' field".into()))?;

        let search_path = input["path"].as_str().unwrap_or(".");

        let full_path = ctx.resolve_existing_path(search_path)?;

        let fixed = input["fixed_strings"].as_bool().unwrap_or(false);

        // Try rg first, fall back to grep
        let (program, args) = if which_exists("rg") {
            let mut args = vec![
                "--line-number".to_string(),
                "--no-heading".to_string(),
                "--color=never".to_string(),
                "--max-count=50".to_string(),
            ];
            if fixed {
                args.push("--fixed-strings".to_string());
            }
            if let Some(include) = input["include"].as_str() {
                args.push("--glob".to_string());
                args.push(include.to_string());
            }
            args.push(pattern.to_string());
            args.push(full_path.to_string_lossy().to_string());
            ("rg", args)
        } else {
            let mut args = vec!["-rn".to_string(), "--color=never".to_string()];
            if fixed {
                args.push("-F".to_string());
            }
            if let Some(include) = input["include"].as_str() {
                args.push("--include".to_string());
                args.push(include.to_string());
            }
            args.push(pattern.to_string());
            args.push(full_path.to_string_lossy().to_string());
            ("grep", args)
        };

        let output = Command::new(program)
            .args(&args)
            .output()
            .await
            .map_err(|e| ToolError::Execution(format!("Failed to run {program}: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        if stdout.is_empty() {
            Ok(ToolOutput::success(format!(
                "No matches found for '{pattern}'"
            )))
        } else {
            let mut result = stdout.to_string();
            if result.len() > 50000 {
                result.truncate(50000);
                result.push_str("\n... (output truncated)");
            }
            Ok(ToolOutput::success(result))
        }
    }
}

fn which_exists(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
