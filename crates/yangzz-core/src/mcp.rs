//! MCP (Model Context Protocol) client implementation.
//!
//! Connects to external MCP servers via stdio transport (JSON-RPC 2.0).
//! Discovers tools from MCP servers and makes them available to the agentic loop.

use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{info, warn};

/// MCP server configuration from mcp.json
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// An MCP tool discovered from a server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub server_name: String,
}

/// JSON-RPC 2.0 message
#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

#[derive(Deserialize, Debug)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: Option<u64>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize, Debug)]
struct JsonRpcError {
    #[allow(dead_code)]
    code: i64,
    message: String,
}

/// Active MCP connection
pub struct McpConnection {
    pub config: McpServerConfig,
    child: Child,
    next_id: u64,
}

impl McpConnection {
    /// Start an MCP server process
    pub async fn start(config: McpServerConfig) -> anyhow::Result<Self> {
        info!("Starting MCP server: {} ({})", config.name, config.command);

        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        for (k, v) in &config.env {
            cmd.env(k, v);
        }

        let child = cmd
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start MCP server '{}': {e}", config.name))?;

        let mut conn = Self {
            config,
            child,
            next_id: 1,
        };

        // Initialize the connection
        conn.initialize().await?;

        Ok(conn)
    }

    /// Send JSON-RPC initialize request
    pub async fn initialize(&mut self) -> anyhow::Result<()> {
        let resp = self
            .send_request(
                "initialize",
                Some(json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "yangzz",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                })),
            )
            .await?;

        info!("MCP server '{}' initialized: {:?}", self.config.name, resp);

        // Send initialized notification
        self.send_notification("notifications/initialized", None)
            .await?;

        Ok(())
    }

    /// List available tools from the MCP server
    pub async fn list_tools(&mut self) -> anyhow::Result<Vec<McpTool>> {
        let resp = self.send_request("tools/list", None).await?;

        let tools_array = resp
            .as_object()
            .and_then(|o| o.get("tools"))
            .and_then(|t| t.as_array())
            .cloned()
            .unwrap_or_default();

        let mut tools = Vec::new();
        for tool_val in tools_array {
            let name = tool_val["name"].as_str().unwrap_or_default().to_string();
            let description = tool_val["description"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let input_schema = tool_val["inputSchema"].clone();

            tools.push(McpTool {
                name,
                description,
                input_schema,
                server_name: self.config.name.clone(),
            });
        }

        info!(
            "MCP server '{}': {} tools discovered",
            self.config.name,
            tools.len()
        );
        Ok(tools)
    }

    /// Call a tool on the MCP server
    pub async fn call_tool(&mut self, name: &str, arguments: &Value) -> anyhow::Result<String> {
        let resp = self
            .send_request(
                "tools/call",
                Some(json!({
                    "name": name,
                    "arguments": arguments,
                })),
            )
            .await?;

        // Extract content from response
        let content = resp
            .as_object()
            .and_then(|o| o.get("content"))
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|| resp.to_string());

        Ok(content)
    }

    /// Send a JSON-RPC request and wait for response
    async fn send_request(&mut self, method: &str, params: Option<Value>) -> anyhow::Result<Value> {
        let id = self.next_id;
        self.next_id += 1;

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let mut request_line = serde_json::to_string(&request)?;
        request_line.push('\n');

        let stdin = self
            .child
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("MCP server stdin closed"))?;
        stdin.write_all(request_line.as_bytes()).await?;
        stdin.flush().await?;

        // Read response line
        let stdout = self
            .child
            .stdout
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("MCP server stdout closed"))?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).await?;

        let response: JsonRpcResponse = serde_json::from_str(&line)?;

        if let Some(error) = response.error {
            return Err(anyhow::anyhow!("MCP error: {}", error.message));
        }

        Ok(response.result.unwrap_or(Value::Null))
    }

    /// Send a JSON-RPC notification (no response expected)
    async fn send_notification(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> anyhow::Result<()> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params.unwrap_or(Value::Null),
        });

        let mut line = serde_json::to_string(&notification)?;
        line.push('\n');

        if let Some(stdin) = self.child.stdin.as_mut() {
            stdin.write_all(line.as_bytes()).await?;
            stdin.flush().await?;
        }

        Ok(())
    }

    pub fn shutdown(&mut self) {
        let _ = self.child.start_kill();
    }
}

impl Drop for McpConnection {
    fn drop(&mut self) {
        // Try to kill the child process
        let _ = self.child.start_kill();
    }
}

/// Load MCP server configs — merge global (~/.yangzz/mcp.json) + project (.yangzz/mcp.json).
/// Project overrides global on name collision.
pub fn load_mcp_configs(cwd: &Path) -> Vec<McpServerConfig> {
    let mut merged: Vec<McpServerConfig> = Vec::new();

    // Global first
    for cfg in load_mcp_configs_at(&crate::paths::yangzz_dir().join("mcp.json")) {
        merged.push(cfg);
    }

    // Project-local: override by name
    for cfg in load_mcp_configs_at(&cwd.join(".yangzz").join("mcp.json")) {
        merged.retain(|c| !c.name.eq_ignore_ascii_case(&cfg.name));
        merged.push(cfg);
    }

    merged
}

/// Load MCP configs from a specific path. Returns empty if file missing.
pub fn load_mcp_configs_at(path: &Path) -> Vec<McpServerConfig> {
    if !path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<McpConfigFile>(&content) {
            Ok(config) => config.servers,
            Err(e) => {
                warn!("Failed to parse {}: {e}", path.display());
                Vec::new()
            }
        },
        Err(e) => {
            warn!("Failed to read {}: {e}", path.display());
            Vec::new()
        }
    }
}

/// Path to the global MCP config file: `~/.yangzz/mcp.json`.
pub fn global_mcp_path() -> std::path::PathBuf {
    crate::paths::yangzz_dir().join("mcp.json")
}

/// Save MCP configs to the global `~/.yangzz/mcp.json`.
pub fn save_global_mcp_configs(configs: &[McpServerConfig]) -> Result<std::path::PathBuf, String> {
    crate::paths::ensure_yangzz_dir();
    let path = global_mcp_path();
    let file = McpConfigFileOwned {
        servers: configs.to_vec(),
    };
    let json = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path)
}

#[derive(Deserialize)]
struct McpConfigFile {
    servers: Vec<McpServerConfig>,
}

#[derive(Serialize)]
struct McpConfigFileOwned {
    servers: Vec<McpServerConfig>,
}

/// MCP Manager: holds all active MCP connections
pub struct McpManager {
    connections: Vec<McpConnection>,
    tools: Vec<McpTool>,
}

impl McpManager {
    /// Initialize MCP from project config
    pub async fn init(cwd: &Path) -> Self {
        let configs = load_mcp_configs(cwd);
        let mut connections = Vec::new();
        let mut tools = Vec::new();

        for config in configs {
            match McpConnection::start(config).await {
                Ok(mut conn) => {
                    match conn.list_tools().await {
                        Ok(server_tools) => {
                            tools.extend(server_tools);
                        }
                        Err(e) => {
                            warn!("Failed to list tools from MCP server: {e}");
                        }
                    }
                    connections.push(conn);
                }
                Err(e) => {
                    warn!("Failed to start MCP server: {e}");
                }
            }
        }

        info!(
            "MCP initialized: {} servers, {} tools",
            connections.len(),
            tools.len()
        );

        Self { connections, tools }
    }

    /// Get all discovered MCP tools (for tool definitions)
    pub fn tool_definitions(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|t| {
                json!({
                    "name": format!("mcp_{}", t.name),
                    "description": format!("[MCP:{}] {}", t.server_name, t.description),
                    "input_schema": t.input_schema,
                })
            })
            .collect()
    }

    /// Call an MCP tool by prefixed name
    pub async fn call_tool(&mut self, name: &str, arguments: &Value) -> anyhow::Result<String> {
        let tool_name = name.strip_prefix("mcp_").unwrap_or(name);

        // Find which server has this tool
        let server_name = self
            .tools
            .iter()
            .find(|t| t.name == tool_name)
            .map(|t| t.server_name.clone())
            .ok_or_else(|| anyhow::anyhow!("MCP tool not found: {tool_name}"))?;

        // Find the connection
        let conn = self
            .connections
            .iter_mut()
            .find(|c| c.config.name == server_name)
            .ok_or_else(|| anyhow::anyhow!("MCP server not connected: {server_name}"))?;

        conn.call_tool(tool_name, arguments).await
    }

    /// Call an MCP tool on a specific server. This avoids collisions when
    /// multiple MCP servers expose the same tool name.
    pub async fn call_tool_on_server(
        &mut self,
        server_name: &str,
        tool_name: &str,
        arguments: &Value,
    ) -> anyhow::Result<String> {
        let conn = self
            .connections
            .iter_mut()
            .find(|c| c.config.name == server_name)
            .ok_or_else(|| anyhow::anyhow!("MCP server not connected: {server_name}"))?;

        conn.call_tool(tool_name, arguments).await
    }

    /// Check if a tool name is an MCP tool
    pub fn is_mcp_tool(&self, name: &str) -> bool {
        name.starts_with("mcp_")
    }

    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Attempt to reconnect a specific MCP server by name.
    /// Kills the old process and starts a fresh one.
    pub async fn reconnect_server(&mut self, server_name: &str) -> anyhow::Result<()> {
        // Find and remove the old connection
        let idx = self
            .connections
            .iter()
            .position(|c| c.config.name == server_name);

        let config = if let Some(idx) = idx {
            let mut old = self.connections.remove(idx);
            old.shutdown();
            old.config.clone()
        } else {
            return Err(anyhow::anyhow!("MCP server '{server_name}' not found"));
        };

        // Start a new connection
        let mut conn = McpConnection::start(config).await?;

        // Re-discover tools
        if let Ok(server_tools) = conn.list_tools().await {
            // Remove old tools from this server and add new ones
            self.tools.retain(|t| t.server_name != server_name);
            self.tools.extend(server_tools);
        }

        self.connections.push(conn);
        info!("MCP server '{}' reconnected successfully", server_name);
        Ok(())
    }
}

#[derive(Clone)]
struct McpRuntimeTool {
    runtime_name: String,
    description: String,
    input_schema: Value,
    server_name: String,
    tool_name: String,
    manager: Arc<Mutex<McpManager>>,
}

impl McpRuntimeTool {
    fn new(tool: &McpTool, manager: Arc<Mutex<McpManager>>) -> Self {
        Self {
            runtime_name: format!(
                "mcp_{}__{}",
                sanitize_tool_name(&tool.server_name),
                sanitize_tool_name(&tool.name)
            ),
            description: format!("[MCP:{}] {}", tool.server_name, tool.description),
            input_schema: tool.input_schema.clone(),
            server_name: tool.server_name.clone(),
            tool_name: tool.name.clone(),
            manager,
        }
    }
}

#[async_trait]
impl Tool for McpRuntimeTool {
    fn name(&self) -> &str {
        &self.runtime_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }

    async fn execute(&self, input: &Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let mut manager = self.manager.lock().await;

        // First attempt
        match manager
            .call_tool_on_server(&self.server_name, &self.tool_name, input)
            .await
        {
            Ok(result) => return Ok(ToolOutput::success(result)),
            Err(e) => {
                let err_msg = e.to_string();
                // If it looks like a connection issue, try to reconnect
                if err_msg.contains("stdin closed")
                    || err_msg.contains("stdout closed")
                    || err_msg.contains("broken pipe")
                    || err_msg.contains("not connected")
                {
                    warn!(
                        "MCP server '{}' connection lost, attempting reconnect...",
                        self.server_name
                    );
                    // Try to reconnect the server
                    if let Err(reconnect_err) = manager.reconnect_server(&self.server_name).await {
                        return Err(ToolError::Execution(format!(
                            "MCP tool '{}:{}' failed and reconnect also failed: {err_msg} (reconnect: {reconnect_err})",
                            self.server_name, self.tool_name
                        )));
                    }
                    // Retry after reconnect
                    match manager
                        .call_tool_on_server(&self.server_name, &self.tool_name, input)
                        .await
                    {
                        Ok(result) => return Ok(ToolOutput::success(result)),
                        Err(e2) => {
                            return Err(ToolError::Execution(format!(
                                "MCP tool '{}:{}' failed after reconnect: {e2}",
                                self.server_name, self.tool_name
                            )));
                        }
                    }
                }
                return Err(ToolError::Execution(format!(
                    "MCP tool '{}:{}' failed: {err_msg}",
                    self.server_name, self.tool_name
                )));
            }
        }
    }
}

fn sanitize_tool_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}

/// Initialize configured MCP servers and expose every discovered MCP tool as a
/// normal runtime `Tool`, so the rest of the system can stay registry-driven.
pub async fn load_mcp_runtime_tools(cwd: &Path) -> Vec<Arc<dyn Tool>> {
    let manager = Arc::new(Mutex::new(McpManager::init(cwd).await));

    let tools_snapshot = {
        let guard = manager.lock().await;
        guard.tools.clone()
    };

    let runtime_tools = tools_snapshot
        .iter()
        .map(|tool| Arc::new(McpRuntimeTool::new(tool, Arc::clone(&manager))) as Arc<dyn Tool>)
        .collect::<Vec<_>>();

    info!("Loaded {} MCP runtime tools", runtime_tools.len());
    runtime_tools
}

/// Health check: start the server, send `initialize`, check for a valid response, then kill it.
/// Returns (server_name, is_healthy, tool_count_or_error_message).
pub async fn check_server_health(config: &McpServerConfig) -> (String, bool, String) {
    match McpConnection::start(config.clone()).await {
        Ok(mut conn) => {
            match conn.initialize().await {
                Ok(_) => {
                    let tools = conn.list_tools().await.unwrap_or_default();
                    let count = tools.len();
                    conn.shutdown();
                    (config.name.clone(), true, format!("{count} tools"))
                }
                Err(e) => {
                    conn.shutdown();
                    (config.name.clone(), false, format!("init failed: {e}"))
                }
            }
        }
        Err(e) => (config.name.clone(), false, format!("start failed: {e}")),
    }
}

/// Check health of all configured MCP servers
pub async fn check_all_servers_health(cwd: &Path) -> Vec<(String, bool, String)> {
    let configs = load_mcp_configs(cwd);
    let mut results = Vec::new();
    for config in &configs {
        results.push(check_server_health(config).await);
    }
    results
}
