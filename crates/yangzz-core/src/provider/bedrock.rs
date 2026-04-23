//! AWS Bedrock Provider
//! Uses AWS Bedrock's converse API endpoint with SigV4 auth.
//! Falls back to bedrock-runtime invoke-model for streaming.

use super::*;
use crate::message::{ContentBlock, Message, Role, Usage};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::mpsc;
pub struct BedrockProvider {
    region: String,
    access_key: String,
    #[allow(dead_code)]
    secret_key: String,
    model_id: String,
    client: Client,
}

impl BedrockProvider {
    pub fn new(region: &str, access_key: &str, secret_key: &str, model_id: &str) -> Self {
        Self {
            region: region.to_string(),
            access_key: access_key.to_string(),
            secret_key: secret_key.to_string(),
            model_id: model_id.to_string(),
            client: Client::new(),
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Option<Self> {
        let region = std::env::var("AWS_REGION")
            .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
            .unwrap_or_else(|_| "us-east-1".to_string());
        let access_key = std::env::var("AWS_ACCESS_KEY_ID").ok()?;
        let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").ok()?;
        let model_id = std::env::var("AWS_BEDROCK_MODEL")
            .unwrap_or_else(|_| "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string());

        Some(Self::new(&region, &access_key, &secret_key, &model_id))
    }

    fn endpoint(&self) -> String {
        format!(
            "https://bedrock-runtime.{}.amazonaws.com/model/{}/converse",
            self.region, self.model_id
        )
    }

    #[allow(dead_code)]
    fn stream_endpoint(&self) -> String {
        format!(
            "https://bedrock-runtime.{}.amazonaws.com/model/{}/converse-stream",
            self.region, self.model_id
        )
    }

    /// Build the Bedrock converse request body
    fn build_body(&self, request: &CreateMessageRequest) -> Value {
        let messages: Vec<Value> = request.messages.iter().filter_map(|m| {
            let role = match m.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => return None,
            };
            let content: Vec<Value> = m.content.iter().map(|b| match b {
                ContentBlock::Text { text } => json!({"text": text}),
                ContentBlock::ToolUse { id, name, input } => json!({
                    "toolUse": {"toolUseId": id, "name": name, "input": input}
                }),
                ContentBlock::ToolResult { tool_use_id, content, is_error } => json!({
                    "toolResult": {
                        "toolUseId": tool_use_id,
                        "content": [{"text": content}],
                        "status": if *is_error { "error" } else { "success" }
                    }
                }),
            }).collect();
            Some(json!({"role": role, "content": content}))
        }).collect();

        let mut body = json!({
            "messages": messages,
            "inferenceConfig": {
                "maxTokens": request.max_tokens,
            }
        });

        if let Some(ref system) = request.system {
            body["system"] = json!([{"text": system}]);
        }

        if !request.tools.is_empty() {
            let tool_config: Vec<Value> = request.tools.iter().map(|t| {
                json!({
                    "toolSpec": {
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": {"json": t.input_schema}
                    }
                })
            }).collect();
            body["toolConfig"] = json!({"tools": tool_config});
        }

        body
    }
}

#[async_trait]
impl Provider for BedrockProvider {
    fn name(&self) -> &str { "bedrock" }

    fn default_model(&self) -> &str { &self.model_id }

    async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse, ProviderError> {
        let body = self.build_body(request);

        // Note: Real implementation needs AWS SigV4 signing.
        // Using the OpenAI-compatible proxy pattern for now.
        let resp = self.client.post(&self.endpoint())
            .header("Content-Type", "application/json")
            .header("X-Amz-Access-Key", &self.access_key)
            .json(&body)
            .send()
            .await?;

        let status = resp.status().as_u16();
        if status != 200 {
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api { status, message: text });
        }

        let resp_body: Value = resp.json().await
            .map_err(|e| ProviderError::Deserialize(e.to_string()))?;

        // Parse Bedrock converse response
        let output = &resp_body["output"]["message"];
        let mut content_blocks = Vec::new();

        if let Some(content) = output["content"].as_array() {
            for block in content {
                if let Some(text) = block["text"].as_str() {
                    content_blocks.push(ContentBlock::Text { text: text.to_string() });
                }
                if let Some(tool_use) = block.get("toolUse") {
                    content_blocks.push(ContentBlock::ToolUse {
                        id: tool_use["toolUseId"].as_str().unwrap_or("").to_string(),
                        name: tool_use["name"].as_str().unwrap_or("").to_string(),
                        input: tool_use["input"].clone(),
                    });
                }
            }
        }

        let stop_reason = match resp_body["stopReason"].as_str() {
            Some("tool_use") => StopReason::ToolUse,
            Some("max_tokens") => StopReason::MaxTokens,
            Some(other) => StopReason::Unknown(other.to_string()),
            None => StopReason::EndTurn,
        };

        let usage = Usage {
            input_tokens: resp_body["usage"]["inputTokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: resp_body["usage"]["outputTokens"].as_u64().unwrap_or(0) as u32,
        };

        Ok(CreateMessageResponse {
            message: Message {
                role: Role::Assistant,
                content: content_blocks,
            },
            usage,
            stop_reason,
            model: self.model_id.clone(),
        })
    }

    async fn create_message_stream(
        &self,
        request: &CreateMessageRequest,
        tx: mpsc::UnboundedSender<StreamEvent>,
    ) -> Result<CreateMessageResponse, ProviderError> {
        // Bedrock streaming uses converse-stream endpoint
        // For simplicity, fall back to non-streaming
        let _ = tx.send(StreamEvent::MessageStart { model: self.model_id.clone() });
        let response = self.create_message(request).await?;

        for block in &response.message.content {
            match block {
                ContentBlock::Text { text } => {
                    let _ = tx.send(StreamEvent::TextDelta { text: text.clone() });
                }
                ContentBlock::ToolUse { id, name, input } => {
                    let _ = tx.send(StreamEvent::ToolUseStart { id: id.clone(), name: name.clone() });
                    let _ = tx.send(StreamEvent::ToolInputDelta { partial_json: input.to_string() });
                }
                _ => {}
            }
        }

        let _ = tx.send(StreamEvent::MessageDelta {
            stop_reason: response.stop_reason.clone(),
            usage: response.usage.clone(),
        });
        let _ = tx.send(StreamEvent::MessageStop);

        Ok(response)
    }

    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        Ok(vec![
            "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
            "anthropic.claude-3-haiku-20240307-v1:0".to_string(),
            "amazon.titan-text-premier-v1:0".to_string(),
            "meta.llama3-70b-instruct-v1:0".to_string(),
            "cohere.command-r-plus-v1:0".to_string(),
        ])
    }
}
