pub mod builtin;
mod executor;
mod registry;

pub use executor::ToolExecutor;
pub use registry::ToolRegistry;

use async_trait::async_trait;
use serde_json::Value;

/// Tool execution context
pub struct ToolContext {
    pub cwd: std::path::PathBuf,
}

/// Tool output
#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub content: String,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn success(content: impl Into<String>) -> Self {
        Self { content: content.into(), is_error: false }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self { content: content.into(), is_error: true }
    }
}

/// Tool errors
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Execution error: {0}")]
    Execution(String),

    #[error("Tool not found: {0}")]
    NotFound(String),
}

/// The Tool trait — one of the 5 核心元
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique tool name
    fn name(&self) -> &str;

    /// Human-readable description
    fn description(&self) -> &str;

    /// JSON Schema for input parameters
    fn input_schema(&self) -> Value;

    /// Is this a read-only operation?
    fn is_read_only(&self) -> bool { false }

    /// Could this destroy data?
    fn is_destructive(&self) -> bool { false }

    /// Execute the tool
    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError>;
}
