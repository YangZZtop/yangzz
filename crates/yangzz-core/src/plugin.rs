//! Plugin system: load external tools from .yangzz/plugins/
//!
//! Each plugin is a directory containing a manifest.json:
//! {
//!   "name": "my-tool",
//!   "description": "Does something",
//!   "command": "python3 tool.py",
//!   "input_schema": { ... }
//! }
//!
//! The plugin receives JSON input on stdin and outputs JSON result on stdout.

use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

#[derive(Deserialize, Debug)]
struct PluginManifest {
    name: String,
    description: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    input_schema: Value,
    #[serde(default)]
    read_only: bool,
}

/// A tool loaded from a plugin directory
pub struct PluginTool {
    manifest: PluginManifest,
    plugin_dir: PathBuf,
}

impl PluginTool {
    fn new(manifest: PluginManifest, plugin_dir: PathBuf) -> Self {
        Self { manifest, plugin_dir }
    }
}

#[async_trait]
impl Tool for PluginTool {
    fn name(&self) -> &str { &self.manifest.name }

    fn description(&self) -> &str { &self.manifest.description }

    fn input_schema(&self) -> Value { self.manifest.input_schema.clone() }

    fn is_read_only(&self) -> bool { self.manifest.read_only }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        use tokio::process::Command;

        let mut cmd = Command::new(&self.manifest.command);
        cmd.args(&self.manifest.args)
            .current_dir(&ctx.cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Set PLUGIN_DIR so the plugin knows where it lives
        cmd.env("YANGZZ_PLUGIN_DIR", &self.plugin_dir);

        let mut child = cmd.spawn()
            .map_err(|e| ToolError::Execution(format!("Plugin '{}' failed to start: {e}", self.manifest.name)))?;

        // Write input JSON to stdin
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let input_json = serde_json::to_string(input).unwrap_or_default();
            let _ = stdin.write_all(input_json.as_bytes()).await;
            drop(stdin);
        }

        let output = child.wait_with_output().await
            .map_err(|e| ToolError::Execution(format!("Plugin '{}' error: {e}", self.manifest.name)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(ToolOutput::success(stdout))
        } else {
            Ok(ToolOutput::error(format!("{stdout}\n{stderr}")))
        }
    }
}

/// Load all plugins from .yangzz/plugins/ directory
pub fn load_plugins(cwd: &Path) -> Vec<Box<dyn Tool>> {
    let plugins_dir = cwd.join(".yangzz").join("plugins");
    if !plugins_dir.exists() {
        return Vec::new();
    }

    let mut tools: Vec<Box<dyn Tool>> = Vec::new();

    let entries = match std::fs::read_dir(&plugins_dir) {
        Ok(e) => e,
        Err(_) => return tools,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let manifest_path = path.join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }

        match std::fs::read_to_string(&manifest_path) {
            Ok(content) => {
                match serde_json::from_str::<PluginManifest>(&content) {
                    Ok(manifest) => {
                        info!("Loaded plugin: {} from {}", manifest.name, path.display());
                        tools.push(Box::new(PluginTool::new(manifest, path)));
                    }
                    Err(e) => {
                        warn!("Invalid plugin manifest at {}: {e}", manifest_path.display());
                    }
                }
            }
            Err(e) => {
                warn!("Cannot read plugin manifest at {}: {e}", manifest_path.display());
            }
        }
    }

    info!("Loaded {} plugins", tools.len());
    tools
}
