use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{Value, json};

pub struct FileEditTool;

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &str {
        "file_edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing an exact string match with new content. The old_string must match exactly."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact string to find and replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The replacement string"
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'path' field".into()))?;
        let old_string = input["old_string"]
            .as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'old_string' field".into()))?;
        let new_string = input["new_string"]
            .as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'new_string' field".into()))?;

        let full_path = ctx.resolve_existing_path(path)?;

        let content = tokio::fs::read_to_string(&full_path).await.map_err(|e| {
            ToolError::Execution(format!("Cannot read {}: {e}", full_path.display()))
        })?;

        let count = content.matches(old_string).count();
        if count == 0 {
            return Err(ToolError::Execution(format!(
                "old_string not found in {}",
                full_path.display()
            )));
        }
        if count > 1 {
            return Err(ToolError::Execution(format!(
                "old_string found {count} times in {} — must be unique",
                full_path.display()
            )));
        }

        let new_content = content.replacen(old_string, new_string, 1);

        tokio::fs::write(&full_path, &new_content)
            .await
            .map_err(|e| {
                ToolError::Execution(format!("Cannot write {}: {e}", full_path.display()))
            })?;

        Ok(ToolOutput::success(format!(
            "Edited {}",
            full_path.display()
        )))
    }
}
