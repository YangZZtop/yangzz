use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::process::Command;

pub struct TreeTool;

#[async_trait]
impl Tool for TreeTool {
    fn name(&self) -> &str {
        "tree"
    }

    fn description(&self) -> &str {
        "Display directory tree structure. Good for understanding project layout."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory to show tree of (default: .)" },
                "max_depth": { "type": "integer", "description": "Maximum depth (default: 3)", "default": 3 },
                "pattern": { "type": "string", "description": "File pattern to include (e.g. '*.rs')" }
            },
            "required": []
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = input["path"].as_str().unwrap_or(".");
        let max_depth = input["max_depth"].as_u64().unwrap_or(3).min(5);

        let full_path = ctx.resolve_existing_path(path)?;

        // Try tree command, fall back to find
        let output = if which_exists("tree") {
            let mut args = vec![
                "-L".to_string(),
                max_depth.to_string(),
                "--noreport".to_string(),
                "-I".to_string(),
                "node_modules|.git|target|__pycache__|.next|dist".to_string(),
            ];
            if let Some(pat) = input["pattern"].as_str() {
                args.push("-P".to_string());
                args.push(pat.to_string());
            }
            args.push(full_path.to_string_lossy().to_string());
            Command::new("tree").args(&args).output().await
        } else {
            Command::new("find")
                .args([
                    full_path.to_string_lossy().as_ref(),
                    "-maxdepth",
                    &max_depth.to_string(),
                    "-not",
                    "-path",
                    "*/node_modules/*",
                    "-not",
                    "-path",
                    "*/.git/*",
                    "-not",
                    "-path",
                    "*/target/*",
                ])
                .output()
                .await
        };

        match output {
            Ok(out) => {
                let mut result = String::from_utf8_lossy(&out.stdout).to_string();
                if result.len() > 50000 {
                    result.truncate(50000);
                    result.push_str("\n... (truncated)");
                }
                Ok(ToolOutput::success(result))
            }
            Err(e) => Err(ToolError::Execution(format!("Failed: {e}"))),
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
