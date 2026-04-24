use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::process::Command;

pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern. Uses fd or find under the hood."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match (e.g. '**/*.rs', 'src/**/*.ts')"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: current directory)"
                }
            },
            "required": ["pattern"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let pattern = input["pattern"]
            .as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'pattern' field".into()))?;

        let search_path = input["path"].as_str().unwrap_or(".");
        let full_path = ctx.resolve_existing_path(search_path)?;

        // Build excludes for macOS TCC-protected dirs so globbing under $HOME
        // doesn't spew permission errors or waste time.
        let tcc_excludes: &[&str] = if cfg!(target_os = "macos") {
            &[
                "Library/Accounts",
                "Library/Mail",
                "Library/Messages",
                "Library/Keychains",
                "Library/Cookies",
                "Library/HomeKit",
                "Library/Suggestions",
                "Library/IdentityServices",
                "Library/Metadata/CoreSpotlight",
                "Library/Application Support/CallHistoryDB",
                "Library/Application Support/CallHistoryTransactions",
                "Library/Application Support/AddressBook",
                "Library/Application Support/com.apple.TCC",
            ]
        } else {
            &[]
        };

        // Try fd first, fall back to find
        let output = if which_exists("fd") {
            let mut args: Vec<String> = vec![
                "--glob".to_string(),
                pattern.to_string(),
                "--max-results".to_string(),
                "200".to_string(),
            ];
            for ex in tcc_excludes {
                args.push("--exclude".to_string());
                args.push((*ex).to_string());
            }
            Command::new("fd")
                .args(&args)
                .current_dir(&full_path)
                .output()
                .await
        } else {
            // Build find invocation: prune protected paths, then name match.
            let mut args: Vec<String> = vec![full_path.to_string_lossy().to_string()];
            for ex in tcc_excludes {
                args.push("-path".to_string());
                args.push(format!("*/{ex}"));
                args.push("-prune".to_string());
                args.push("-o".to_string());
            }
            args.extend([
                "-name".to_string(),
                pattern.to_string(),
                "-maxdepth".to_string(),
                "10".to_string(),
                "-print".to_string(),
            ]);
            Command::new("find").args(&args).output().await
        };

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.trim().is_empty() {
                    Ok(ToolOutput::success(format!(
                        "No files matching '{pattern}'"
                    )))
                } else {
                    let lines: Vec<&str> = stdout.lines().collect();
                    let count = lines.len();
                    let mut result = lines.join("\n");
                    if result.len() > 50000 {
                        result.truncate(50000);
                        result.push_str("\n... (truncated)");
                    }
                    Ok(ToolOutput::success(format!(
                        "{count} files found:\n{result}"
                    )))
                }
            }
            Err(e) => Err(ToolError::Execution(format!("Failed to search: {e}"))),
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
