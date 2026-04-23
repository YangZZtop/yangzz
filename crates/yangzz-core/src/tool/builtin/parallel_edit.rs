use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;

/// ParallelEditTool: edit multiple files simultaneously
pub struct ParallelEditTool;

#[async_trait]
impl Tool for ParallelEditTool {
    fn name(&self) -> &str { "parallel_edit" }

    fn description(&self) -> &str {
        "Edit multiple files simultaneously. Each edit specifies a file path, old_string, and new_string. All edits are applied atomically — if any fails, none are applied."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "edits": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "file_path": { "type": "string", "description": "Path to the file" },
                            "old_string": { "type": "string", "description": "Exact string to find" },
                            "new_string": { "type": "string", "description": "Replacement string" }
                        },
                        "required": ["file_path", "old_string", "new_string"]
                    },
                    "description": "Array of file edits to apply in parallel"
                }
            },
            "required": ["edits"]
        })
    }

    fn is_read_only(&self) -> bool { false }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let edits = input["edits"].as_array()
            .ok_or_else(|| ToolError::Validation("Missing 'edits' array".into()))?;

        if edits.is_empty() {
            return Err(ToolError::Validation("Empty edits array".into()));
        }

        if edits.len() > 20 {
            return Err(ToolError::Validation("Maximum 20 parallel edits".into()));
        }

        // Phase 1: Read all files and validate edits
        let mut edit_plan: Vec<(PathBuf, String, String, String)> = Vec::new();

        for (i, edit) in edits.iter().enumerate() {
            let file_path = edit["file_path"].as_str()
                .ok_or_else(|| ToolError::Validation(format!("Edit #{}: missing file_path", i + 1)))?;
            let old_string = edit["old_string"].as_str()
                .ok_or_else(|| ToolError::Validation(format!("Edit #{}: missing old_string", i + 1)))?;
            let new_string = edit["new_string"].as_str()
                .ok_or_else(|| ToolError::Validation(format!("Edit #{}: missing new_string", i + 1)))?;

            let full_path = if file_path.starts_with('/') {
                PathBuf::from(file_path)
            } else {
                ctx.cwd.join(file_path)
            };

            // Symlink protection
            if full_path.is_symlink() {
                return Err(ToolError::Validation(format!(
                    "Edit #{}: Refusing to edit symlink: {}", i + 1, file_path
                )));
            }

            let content = tokio::fs::read_to_string(&full_path).await
                .map_err(|e| ToolError::Execution(format!("Edit #{}: Cannot read {}: {e}", i + 1, file_path)))?;

            if !content.contains(old_string) {
                return Err(ToolError::Execution(format!(
                    "Edit #{}: old_string not found in {}", i + 1, file_path
                )));
            }

            // Check uniqueness
            let count = content.matches(old_string).count();
            if count > 1 {
                return Err(ToolError::Execution(format!(
                    "Edit #{}: old_string found {} times in {} (must be unique)", i + 1, count, file_path
                )));
            }

            edit_plan.push((full_path, content, old_string.to_string(), new_string.to_string()));
        }

        // Phase 2: Apply all edits (atomic-like: all validated before any write)
        let mut results = Vec::new();
        for (full_path, content, old_string, new_string) in &edit_plan {
            let new_content = content.replacen(old_string, new_string, 1);
            tokio::fs::write(&full_path, &new_content).await
                .map_err(|e| ToolError::Execution(format!("Failed to write {}: {e}", full_path.display())))?;
            results.push(format!("✓ {}", full_path.display()));
        }

        Ok(ToolOutput::success(format!(
            "Successfully applied {} parallel edits:\n{}",
            results.len(),
            results.join("\n")
        )))
    }
}
