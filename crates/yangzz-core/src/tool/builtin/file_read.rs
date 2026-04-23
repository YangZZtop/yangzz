use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str { "file_read" }

    fn description(&self) -> &str {
        "Read the contents of a file. Can read specific line ranges."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Starting line number (1-indexed)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Number of lines to read"
                }
            },
            "required": ["path"]
        })
    }

    fn is_read_only(&self) -> bool { true }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'path' field".into()))?;

        let full_path = if std::path::Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            ctx.cwd.join(path)
        };

        let content = tokio::fs::read_to_string(&full_path)
            .await
            .map_err(|e| ToolError::Execution(format!("Cannot read {}: {e}", full_path.display())))?;

        let lines: Vec<&str> = content.lines().collect();
        let total = lines.len();

        // First-read: if no offset/limit given, read whole file (avoid fragmented reads)
        let has_range = input.get("offset").is_some() || input.get("limit").is_some();
        let offset = input["offset"].as_u64().map(|o| o as usize).unwrap_or(1);
        let limit = if has_range {
            input["limit"].as_u64().map(|l| l as usize).unwrap_or(total)
        } else {
            // Full file, but cap at 2000 lines to prevent context overflow
            total.min(2000)
        };

        let start = offset.saturating_sub(1).min(total);
        let end = (start + limit).min(total);

        let mut result = String::new();
        for (i, line) in lines[start..end].iter().enumerate() {
            let line_num = start + i + 1;
            result.push_str(&format!("{line_num:>6}\t{line}\n"));
        }

        if result.is_empty() {
            result = "(empty file)".to_string();
        } else {
            result = format!("File: {} ({total} lines)\n{result}", full_path.display());
            if !has_range && total > 2000 {
                result.push_str(&format!("\n... (showing first 2000 of {total} lines, use offset/limit for more)"));
            }
        }

        Ok(ToolOutput::success(result))
    }
}
