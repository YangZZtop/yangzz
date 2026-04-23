use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct NotebookReadTool;
pub struct NotebookEditTool;

#[async_trait]
impl Tool for NotebookReadTool {
    fn name(&self) -> &str { "notebook_read" }

    fn description(&self) -> &str {
        "Read a Jupyter notebook (.ipynb) and display cells with their types, IDs, and content."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the .ipynb file" }
            },
            "required": ["path"]
        })
    }

    fn is_read_only(&self) -> bool { true }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = input["path"].as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'path'".into()))?;

        let full_path = if std::path::Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            ctx.cwd.join(path)
        };

        let content = tokio::fs::read_to_string(&full_path).await
            .map_err(|e| ToolError::Execution(format!("Cannot read {}: {e}", full_path.display())))?;

        let nb: Value = serde_json::from_str(&content)
            .map_err(|e| ToolError::Execution(format!("Invalid notebook JSON: {e}")))?;

        let cells = nb["cells"].as_array()
            .ok_or_else(|| ToolError::Execution("No cells array in notebook".into()))?;

        let mut result = format!("Notebook: {} ({} cells)\n\n", full_path.display(), cells.len());

        for (i, cell) in cells.iter().enumerate() {
            let cell_type = cell["cell_type"].as_str().unwrap_or("unknown");
            let source = cell["source"].as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(""))
                .unwrap_or_default();

            result.push_str(&format!("--- Cell {i} [{cell_type}] ---\n"));
            result.push_str(&source);
            if !source.ends_with('\n') { result.push('\n'); }

            // Show outputs for code cells
            if cell_type == "code" {
                if let Some(outputs) = cell["outputs"].as_array() {
                    for output in outputs {
                        if let Some(text) = output["text"].as_array() {
                            let t: String = text.iter().filter_map(|v| v.as_str()).collect();
                            result.push_str(&format!("[output] {t}"));
                        }
                    }
                }
            }
            result.push('\n');
        }

        if result.len() > 50000 {
            result.truncate(50000);
            result.push_str("\n... (truncated)");
        }

        Ok(ToolOutput::success(result))
    }
}

#[async_trait]
impl Tool for NotebookEditTool {
    fn name(&self) -> &str { "notebook_edit" }

    fn description(&self) -> &str {
        "Edit a Jupyter notebook cell by replacing its source content."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the .ipynb file" },
                "cell_number": { "type": "integer", "description": "0-indexed cell number" },
                "new_source": { "type": "string", "description": "New source content for the cell" },
                "cell_type": { "type": "string", "description": "Cell type: 'code' or 'markdown' (only for insert)" },
                "mode": { "type": "string", "description": "'replace' (default) or 'insert'" }
            },
            "required": ["path", "cell_number", "new_source"]
        })
    }

    fn is_destructive(&self) -> bool { true }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = input["path"].as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'path'".into()))?;
        let cell_num = input["cell_number"].as_u64()
            .ok_or_else(|| ToolError::Validation("Missing 'cell_number'".into()))? as usize;
        let new_source = input["new_source"].as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'new_source'".into()))?;
        let mode = input["mode"].as_str().unwrap_or("replace");

        let full_path = if std::path::Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            ctx.cwd.join(path)
        };

        let content = tokio::fs::read_to_string(&full_path).await
            .map_err(|e| ToolError::Execution(format!("Cannot read: {e}")))?;

        let mut nb: Value = serde_json::from_str(&content)
            .map_err(|e| ToolError::Execution(format!("Invalid JSON: {e}")))?;

        let cells = nb["cells"].as_array_mut()
            .ok_or_else(|| ToolError::Execution("No cells array".into()))?;

        // Convert source to array of lines
        let source_lines: Vec<Value> = new_source.lines()
            .map(|l| Value::String(format!("{l}\n")))
            .collect();

        if mode == "insert" {
            let cell_type = input["cell_type"].as_str().unwrap_or("code");
            let new_cell = json!({
                "cell_type": cell_type,
                "source": source_lines,
                "metadata": {},
                "outputs": if cell_type == "code" { json!([]) } else { json!(null) }
            });
            if cell_num > cells.len() {
                cells.push(new_cell);
            } else {
                cells.insert(cell_num, new_cell);
            }
            Ok(ToolOutput::success(format!("Inserted cell at position {cell_num}")))
        } else {
            if cell_num >= cells.len() {
                return Err(ToolError::Execution(format!("Cell {cell_num} out of range (0..{})", cells.len())));
            }
            cells[cell_num]["source"] = Value::Array(source_lines);
            Ok(ToolOutput::success(format!("Replaced cell {cell_num}")))
        }
        .and_then(|output| {
            // Write back
            let json = serde_json::to_string_pretty(&nb)
                .map_err(|e| ToolError::Execution(format!("JSON serialize error: {e}")))?;
            // Use blocking write since we need the result
            std::fs::write(&full_path, json)
                .map_err(|e| ToolError::Execution(format!("Cannot write: {e}")))?;
            Ok(output)
        })
    }
}
