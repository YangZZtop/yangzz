use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{Value, json};

pub struct MultiEditTool;

#[async_trait]
impl Tool for MultiEditTool {
    fn name(&self) -> &str {
        "multi_edit"
    }

    fn description(&self) -> &str {
        "Make multiple edits to a single file in one operation. Each edit is a find-and-replace. All edits are applied sequentially."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file" },
                "edits": {
                    "type": "array",
                    "description": "Array of {old_string, new_string} edits to apply sequentially",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old_string": { "type": "string" },
                            "new_string": { "type": "string" }
                        },
                        "required": ["old_string", "new_string"]
                    }
                }
            },
            "required": ["path", "edits"]
        })
    }

    fn is_destructive(&self) -> bool {
        true
    }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'path'".into()))?;
        let edits = input["edits"]
            .as_array()
            .ok_or_else(|| ToolError::Validation("Missing 'edits' array".into()))?;

        let full_path = ctx.resolve_existing_path(path)?;

        let mut content = tokio::fs::read_to_string(&full_path).await.map_err(|e| {
            ToolError::Execution(format!("Cannot read {}: {e}", full_path.display()))
        })?;

        let mut applied = 0;
        for (i, edit) in edits.iter().enumerate() {
            let old = edit["old_string"]
                .as_str()
                .ok_or_else(|| ToolError::Validation(format!("Edit {i}: missing old_string")))?;
            let new = edit["new_string"]
                .as_str()
                .ok_or_else(|| ToolError::Validation(format!("Edit {i}: missing new_string")))?;

            let count = content.matches(old).count();
            if count == 0 {
                return Err(ToolError::Execution(format!(
                    "Edit {i}: old_string not found in {} (after {} edits applied)",
                    full_path.display(),
                    applied
                )));
            }
            if count > 1 {
                return Err(ToolError::Execution(format!(
                    "Edit {i}: old_string found {count} times — must be unique"
                )));
            }
            content = content.replacen(old, new, 1);
            applied += 1;
        }

        tokio::fs::write(&full_path, &content)
            .await
            .map_err(|e| ToolError::Execution(format!("Cannot write: {e}")))?;

        Ok(ToolOutput::success(format!(
            "Applied {applied} edits to {}",
            full_path.display()
        )))
    }
}
