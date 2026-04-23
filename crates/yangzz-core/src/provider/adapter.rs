//! Model Adapter Architecture: normalize different API formats into a unified interface.
//! Each adapter handles the specific quirks of a provider's API.

use serde_json::Value;
use tracing::info;

/// Adapter trait for normalizing model-specific behavior
pub trait ModelAdapter: Send + Sync {
    /// Adapter name
    fn name(&self) -> &str;

    /// Transform system prompt for this model's format
    fn adapt_system_prompt(&self, system: &str) -> String {
        system.to_string()
    }

    /// Maximum context window for this model
    fn context_window(&self) -> usize;

    /// Whether this model supports tool use
    fn supports_tools(&self) -> bool { true }

    /// Whether this model supports streaming
    fn supports_streaming(&self) -> bool { true }

    /// Whether this model supports vision/images
    fn supports_vision(&self) -> bool { false }

    /// Recommended max output tokens
    fn default_max_tokens(&self) -> u32 { 4096 }

    /// Transform tool definitions for this model
    fn adapt_tools(&self, tools: &[Value]) -> Vec<Value> {
        tools.to_vec()
    }
}

/// OpenAI family adapter (GPT-4o, GPT-5, o3, etc.)
pub struct OpenAiAdapter {
    pub model: String,
}

impl ModelAdapter for OpenAiAdapter {
    fn name(&self) -> &str { "openai" }

    fn context_window(&self) -> usize {
        if self.model.contains("gpt-5") { 1_000_000 }
        else if self.model.contains("o3") || self.model.contains("o4") { 200_000 }
        else if self.model.contains("gpt-4o") { 128_000 }
        else { 128_000 }
    }

    fn supports_vision(&self) -> bool {
        self.model.contains("gpt-4o") || self.model.contains("gpt-5") || self.model.contains("o4")
    }

    fn default_max_tokens(&self) -> u32 {
        if self.model.contains("gpt-5") { 32_000 }
        else { 16_384 }
    }
}

/// Anthropic family adapter (Claude 3.5/4/etc.)
pub struct AnthropicAdapter {
    pub model: String,
}

impl ModelAdapter for AnthropicAdapter {
    fn name(&self) -> &str { "anthropic" }

    fn context_window(&self) -> usize {
        if self.model.contains("claude-4") { 1_000_000 }
        else { 200_000 }
    }

    fn supports_vision(&self) -> bool { true }

    fn default_max_tokens(&self) -> u32 { 8_192 }
}

/// Google Gemini adapter
pub struct GeminiAdapter {
    pub model: String,
}

impl ModelAdapter for GeminiAdapter {
    fn name(&self) -> &str { "gemini" }

    fn context_window(&self) -> usize {
        if self.model.contains("2.5") { 1_000_000 }
        else { 1_000_000 }
    }

    fn supports_vision(&self) -> bool { true }

    fn default_max_tokens(&self) -> u32 { 8_192 }
}

/// DeepSeek adapter
pub struct DeepSeekAdapter {
    pub model: String,
}

impl ModelAdapter for DeepSeekAdapter {
    fn name(&self) -> &str { "deepseek" }

    fn context_window(&self) -> usize { 64_000 }

    fn default_max_tokens(&self) -> u32 { 8_192 }
}

/// Ollama/local model adapter
pub struct OllamaAdapter {
    pub model: String,
}

impl ModelAdapter for OllamaAdapter {
    fn name(&self) -> &str { "ollama" }

    fn context_window(&self) -> usize { 32_000 }

    fn supports_tools(&self) -> bool { false }

    fn supports_vision(&self) -> bool { false }

    fn default_max_tokens(&self) -> u32 { 4_096 }
}

/// AWS Bedrock adapter
pub struct BedrockAdapter {
    pub model: String,
}

impl ModelAdapter for BedrockAdapter {
    fn name(&self) -> &str { "bedrock" }

    fn context_window(&self) -> usize {
        if self.model.contains("claude") { 200_000 }
        else if self.model.contains("titan") { 32_000 }
        else { 128_000 }
    }

    fn default_max_tokens(&self) -> u32 { 4_096 }
}

/// GCP Vertex AI adapter
pub struct VertexAdapter {
    pub model: String,
}

impl ModelAdapter for VertexAdapter {
    fn name(&self) -> &str { "vertex" }

    fn context_window(&self) -> usize {
        if self.model.contains("gemini") { 1_000_000 }
        else if self.model.contains("claude") { 200_000 }
        else { 128_000 }
    }

    fn default_max_tokens(&self) -> u32 { 8_192 }
}

/// Resolve the appropriate adapter for a model/provider combination
pub fn resolve_adapter(provider: &str, model: &str) -> Box<dyn ModelAdapter> {
    info!("Resolving adapter for provider={provider}, model={model}");
    match provider {
        "anthropic" => Box::new(AnthropicAdapter { model: model.to_string() }),
        "gemini" | "google" => Box::new(GeminiAdapter { model: model.to_string() }),
        "deepseek" => Box::new(DeepSeekAdapter { model: model.to_string() }),
        "ollama" => Box::new(OllamaAdapter { model: model.to_string() }),
        "bedrock" => Box::new(BedrockAdapter { model: model.to_string() }),
        "vertex" => Box::new(VertexAdapter { model: model.to_string() }),
        _ => Box::new(OpenAiAdapter { model: model.to_string() }),
    }
}
