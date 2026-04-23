use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;

pub struct ListDirTool;

#[async_trait]
impl Tool for ListDirTool {
    fn name(&self) -> &str { "list_dir" }

    fn description(&self) -> &str {
        "List files and directories in a given path. Shows file sizes and directory item counts."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path to list (default: current directory)"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum depth to recurse (default: 1, max: 3)"
                }
            },
            "required": []
        })
    }

    fn is_read_only(&self) -> bool { true }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = input["path"].as_str().unwrap_or(".");
        let max_depth = input["max_depth"].as_u64().unwrap_or(1).min(3) as usize;

        let full_path = if Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            ctx.cwd.join(path)
        };

        // Symlink protection
        let canonical = full_path.canonicalize()
            .map_err(|e| ToolError::Execution(format!("Cannot resolve path {}: {e}", full_path.display())))?;
        if !canonical.starts_with(&ctx.cwd) && !full_path.is_absolute() {
            return Err(ToolError::Execution("Path escapes project directory via symlink".into()));
        }

        let mut result = String::new();
        list_recursive(&canonical, &canonical, 0, max_depth, &mut result)?;

        if result.is_empty() {
            result = "(empty directory)".to_string();
        } else {
            result = format!("Directory: {}\n{result}", canonical.display());
        }

        Ok(ToolOutput::success(result))
    }
}

fn list_recursive(
    base: &Path,
    dir: &Path,
    depth: usize,
    max_depth: usize,
    out: &mut String,
) -> Result<(), ToolError> {
    if depth > max_depth { return Ok(()); }

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| ToolError::Execution(format!("Cannot read {}: {e}", dir.display())))?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let indent = "  ".repeat(depth);
    let mut count = 0;

    for entry in entries {
        if count >= 200 { 
            out.push_str(&format!("{indent}  ... (truncated)\n"));
            break;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden files at depth 0
        if depth == 0 && name.starts_with('.') && name != ".gitignore" { continue; }

        let meta = entry.metadata().ok();
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);

        if is_dir {
            let child_count = std::fs::read_dir(entry.path())
                .map(|rd| rd.count())
                .unwrap_or(0);
            out.push_str(&format!("{indent}  {name}/ ({child_count} items)\n"));
            if depth + 1 <= max_depth {
                list_recursive(base, &entry.path(), depth + 1, max_depth, out)?;
            }
        } else {
            let size = meta.map(|m| m.len()).unwrap_or(0);
            let size_str = if size < 1024 {
                format!("{size} B")
            } else if size < 1024 * 1024 {
                format!("{:.1} KB", size as f64 / 1024.0)
            } else {
                format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
            };
            out.push_str(&format!("{indent}  {name} ({size_str})\n"));
        }
        count += 1;
    }
    Ok(())
}
