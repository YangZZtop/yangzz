//! MCP (Model Context Protocol) client implementation.
//!
//! Connects to external MCP servers via stdio transport (JSON-RPC 2.0).
//! Discovers tools from MCP servers and makes them available to the agentic loop.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tracing::{info, warn};

/// MCP server configuration from .yangzz/mcp.json
#[derive(Debug, Deserialize, Clone)]
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

        let child = cmd.spawn()
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
    async fn initialize(&mut self) -> anyhow::Result<()> {
        let resp = self.send_request("initialize", Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "yangzz",
                "version": env!("CARGO_PKG_VERSION")
            }
        }))).await?;

        info!("MCP server '{}' initialized: {:?}", self.config.name, resp);

        // Send initialized notification
        self.send_notification("notifications/initialized", None).await?;

        Ok(())
    }

    /// List available tools from the MCP server
    pub async fn list_tools(&mut self) -> anyhow::Result<Vec<McpTool>> {
        let resp = self.send_request("tools/list", None).await?;

        let tools_array = resp.as_object()
            .and_then(|o| o.get("tools"))
            .and_then(|t| t.as_array())
            .cloned()
            .unwrap_or_default();

        let mut tools = Vec::new();
        for tool_val in tools_array {
            let name = tool_val["name"].as_str().unwrap_or_default().to_string();
            let description = tool_val["description"].as_str().unwrap_or_default().to_string();
            let input_schema = tool_val["inputSchema"].clone();

            tools.push(McpTool {
                name,
                description,
                input_schema,
                server_name: self.config.name.clone(),
            });
        }

        info!("MCP server '{}': {} tools discovered", self.config.name, tools.len());
        Ok(tools)
    }

    /// Call a tool on the MCP server
    pub async fn call_tool(&mut self, name: &str, arguments: &Value) -> anyhow::Result<String> {
        let resp = self.send_request("tools/call", Some(json!({
            "name": name,
            "arguments": arguments,
        }))).await?;

        // Extract content from response
        let content = resp.as_object()
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

        let stdin = self.child.stdin.as_mut()
            .ok_or_else(|| anyhow::anyhow!("MCP server stdin closed"))?;
        stdin.write_all(request_line.as_bytes()).await?;
        stdin.flush().await?;

        // Read response line
        let stdout = self.child.stdout.as_mut()
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
    async fn send_notification(&mut self, method: &str, params: Option<Value>) -> anyhow::Result<()> {
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
}

impl Drop for McpConnection {
    fn drop(&mut self) {
        // Try to kill the child process
        let _ = self.child.start_kill();
    }
}

/// Load MCP server configs from .yangzz/mcp.json
pub fn load_mcp_configs(cwd: &Path) -> Vec<McpServerConfig> {
    let config_path = cwd.join(".yangzz").join("mcp.json");
    if !config_path.exists() {
        return Vec::new();
    }

    match std::fs::read_to_string(&config_path) {
        Ok(content) => {
            match serde_json::from_str::<McpConfigFile>(&content) {
                Ok(config) => config.servers,
                Err(e) => {
                    warn!("Failed to parse mcp.json: {e}");
                    Vec::new()
                }
            }
        }
        Err(e) => {
            warn!("Failed to read mcp.json: {e}");
            Vec::new()
        }
    }
}

#[derive(Deserialize)]
struct McpConfigFile {
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

        info!("MCP initialized: {} servers, {} tools", connections.len(), tools.len());

        Self { connections, tools }
    }

    /// Get all discovered MCP tools (for tool definitions)
    pub fn tool_definitions(&self) -> Vec<Value> {
        self.tools.iter().map(|t| {
            json!({
                "name": format!("mcp_{}", t.name),
                "description": format!("[MCP:{}] {}", t.server_name, t.description),
                "input_schema": t.input_schema,
            })
        }).collect()
    }

    /// Call an MCP tool by prefixed name
    pub async fn call_tool(&mut self, name: &str, arguments: &Value) -> anyhow::Result<String> {
        let tool_name = name.strip_prefix("mcp_").unwrap_or(name);

        // Find which server has this tool
        let server_name = self.tools.iter()
            .find(|t| t.name == tool_name)
            .map(|t| t.server_name.clone())
            .ok_or_else(|| anyhow::anyhow!("MCP tool not found: {tool_name}"))?;

        // Find the connection
        let conn = self.connections.iter_mut()
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
}
