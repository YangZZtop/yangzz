use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Command;

pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str { "glob" }

    fn description(&self) -> &str {
        "Find files matching a glob pattern. Uses fd or find under the hood."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match (e.g. '**/*.rs', 'src/**/*.ts')"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: current directory)"
                }
            },
            "required": ["pattern"]
        })
    }

    fn is_read_only(&self) -> bool { true }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let pattern = input["pattern"]
            .as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'pattern' field".into()))?;

        let search_path = input["path"].as_str().unwrap_or(".");
        let full_path = if std::path::Path::new(search_path).is_absolute() {
            std::path::PathBuf::from(search_path)
        } else {
            ctx.cwd.join(search_path)
        };

        // Try fd first, fall back to find
        let output = if which_exists("fd") {
            Command::new("fd")
                .args(["--glob", pattern, "--max-results", "200"])
                .current_dir(&full_path)
                .output()
                .await
        } else {
            Command::new("find")
                .args([full_path.to_string_lossy().as_ref(), "-name", pattern, "-maxdepth", "10"])
                .output()
                .await
        };

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.trim().is_empty() {
                    Ok(ToolOutput::success(format!("No files matching '{pattern}'")))
                } else {
                    let lines: Vec<&str> = stdout.lines().collect();
                    let count = lines.len();
                    let mut result = lines.join("\n");
                    if result.len() > 50000 {
                        result.truncate(50000);
                        result.push_str("\n... (truncated)");
                    }
                    Ok(ToolOutput::success(format!("{count} files found:\n{result}")))
                }
            }
            Err(e) => Err(ToolError::Execution(format!("Failed to search: {e}"))),
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
