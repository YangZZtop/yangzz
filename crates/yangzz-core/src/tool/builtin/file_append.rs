use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct FileAppendTool;

#[async_trait]
impl Tool for FileAppendTool {
    fn name(&self) -> &str { "file_append" }

    fn description(&self) -> &str {
        "Append content to the end of an existing file."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file" },
                "content": { "type": "string", "description": "Content to append" }
            },
            "required": ["path", "content"]
        })
    }

    fn is_destructive(&self) -> bool { true }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = input["path"].as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'path'".into()))?;
        let content = input["content"].as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'content'".into()))?;

        let full_path = if std::path::Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            ctx.cwd.join(path)
        };

        // Symlink check
        if full_path.is_symlink() {
            return Err(ToolError::Execution("Refusing to write through symlink".into()));
        }

        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&full_path)
            .await
            .map_err(|e| ToolError::Execution(format!("Cannot open {}: {e}", full_path.display())))?;

        file.write_all(content.as_bytes()).await
            .map_err(|e| ToolError::Execution(format!("Cannot write: {e}")))?;

        let lines = content.lines().count();
        Ok(ToolOutput::success(format!("Appended {lines} lines to {}", full_path.display())))
    }
}
