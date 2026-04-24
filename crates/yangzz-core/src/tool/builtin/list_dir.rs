use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::path::Path;

pub struct ListDirTool;

#[async_trait]
impl Tool for ListDirTool {
    fn name(&self) -> &str {
        "list_dir"
    }

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

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = input["path"].as_str().unwrap_or(".");
        let max_depth = input["max_depth"].as_u64().unwrap_or(1).min(3) as usize;

        let canonical = ctx.resolve_existing_path(path)?;

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

/// Return true if `path` is a known macOS TCC-protected directory where
/// plain file-system access will hit `Operation not permitted` (os error 1).
/// Listing these just wastes time and litters the transcript with errors.
fn is_tcc_protected(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        let s = path.to_string_lossy();
        const BLOCKED: &[&str] = &[
            "/Library/Accounts",
            "/Library/Mail",
            "/Library/Messages",
            "/Library/Keychains",
            "/Library/Cookies",
            "/Library/HomeKit",
            "/Library/Suggestions",
            "/Library/Metadata/CoreSpotlight",
            "/Library/IdentityServices",
            "/Library/Application Support/CallHistoryDB",
            "/Library/Application Support/CallHistoryTransactions",
            "/Library/Application Support/AddressBook",
            "/Library/Application Support/com.apple.TCC",
        ];
        for b in BLOCKED {
            if s.ends_with(b) || s.contains(&format!("{b}/")) {
                return true;
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
    }
    false
}

fn is_permission_denied(e: &std::io::Error) -> bool {
    matches!(e.kind(), std::io::ErrorKind::PermissionDenied) || e.raw_os_error() == Some(1) // EPERM
}

fn list_recursive(
    base: &Path,
    dir: &Path,
    depth: usize,
    max_depth: usize,
    out: &mut String,
) -> Result<(), ToolError> {
    if depth > max_depth {
        return Ok(());
    }

    // macOS TCC-protected paths deny access with `Operation not permitted`.
    // Silently skip those instead of erroring the whole operation.
    if is_tcc_protected(dir) {
        out.push_str(&format!("  (skipped: OS-protected path)\n"));
        return Ok(());
    }

    let mut entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(it) => it.filter_map(|e| e.ok()).collect(),
        Err(e) if is_permission_denied(&e) => {
            // Silent skip on EACCES / EPERM — common for macOS protected dirs
            // when the agent recursively explores $HOME.
            out.push_str(&format!("  (skipped: permission denied)\n"));
            return Ok(());
        }
        Err(e) => {
            return Err(ToolError::Execution(format!(
                "Cannot read {}: {e}",
                dir.display()
            )));
        }
    };
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
        if depth == 0 && name.starts_with('.') && name != ".gitignore" {
            continue;
        }

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
