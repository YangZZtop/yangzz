pub mod adapter;
mod anthropic;
pub mod bedrock;
mod openai_compat;
pub mod router;
mod transport;
pub mod vertex;

pub use anthropic::AnthropicProvider;
pub use bedrock::BedrockProvider;
pub use openai_compat::OpenAiCompatProvider;
pub use vertex::VertexProvider;

use crate::message::{Message, Usage};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Request to create a message
#[derive(Debug, Clone)]
pub struct CreateMessageRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub system: Option<String>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub tools: Vec<ToolDefinition>,
}

/// Tool definition sent to the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Response from creating a message
#[derive(Debug, Clone)]
pub struct CreateMessageResponse {
    pub message: Message,
    pub usage: Usage,
    pub stop_reason: StopReason,
    pub model: String,
}

/// Why the model stopped generating
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    Unknown(String),
}

/// Streaming events from the API
#[derive(Debug, Clone)]
pub enum StreamEvent {
    MessageStart { model: String },
    ContentBlockStart { index: usize },
    TextDelta { text: String },
    ToolUseStart { id: String, name: String },
    ToolInputDelta { partial_json: String },
    ContentBlockStop { index: usize },
    MessageDelta { stop_reason: StopReason, usage: Usage },
    MessageStop,
    Error { message: String },
}

/// Provider errors
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Rate limited, retry after {retry_after_secs:?}s")]
    RateLimit { retry_after_secs: Option<u64> },

    #[error("Deserialization error: {0}")]
    Deserialize(String),

    #[error("Stream error: {0}")]
    Stream(String),

    #[error("{0}")]
    Other(String),
}

/// The core Provider trait — one of the 5 核心元
#[async_trait]
pub trait Provider: Send + Sync {
    /// Provider name (e.g. "anthropic", "openai")
    fn name(&self) -> &str;

    /// Default model for this provider
    fn default_model(&self) -> &str;

    /// Non-streaming message creation
    async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse, ProviderError>;

    /// Streaming message creation
    async fn create_message_stream(
        &self,
        request: &CreateMessageRequest,
        tx: mpsc::UnboundedSender<StreamEvent>,
    ) -> Result<CreateMessageResponse, ProviderError>;

    /// List available models from the provider API
    /// Returns Vec of model IDs. Default: empty (uses hardcoded fallback).
    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        Ok(vec![])
    }
}

/// Which API format to use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiFormat {
    Anthropic,
    OpenAi,
    Gemini,
}
