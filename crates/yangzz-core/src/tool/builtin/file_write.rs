use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct FileWriteTool;

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str { "file_write" }

    fn description(&self) -> &str {
        "Create a new file or overwrite an existing file with the given content."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to create or overwrite"
                },
                "content": {
                    "type": "string",
                    "description": "The full content to write"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn is_destructive(&self) -> bool { true }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'path' field".into()))?;
        let content = input["content"]
            .as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'content' field".into()))?;

        let full_path = if std::path::Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            ctx.cwd.join(path)
        };

        // Symlink protection
        if full_path.exists() && full_path.is_symlink() {
            return Err(ToolError::Execution("Refusing to write through symlink".into()));
        }

        // Create parent directories if needed
        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ToolError::Execution(format!("Cannot create directory: {e}")))?;
        }

        tokio::fs::write(&full_path, content)
            .await
            .map_err(|e| ToolError::Execution(format!("Cannot write {}: {e}", full_path.display())))?;

        let lines = content.lines().count();
        Ok(ToolOutput::success(format!("Wrote {} ({lines} lines)", full_path.display())))
    }
}
