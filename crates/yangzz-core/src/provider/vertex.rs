//! GCP Vertex AI Provider
//! Supports Claude and Gemini models via Vertex AI endpoint.

use super::*;
use crate::message::{ContentBlock, Message, Role, Usage};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::mpsc;


pub struct VertexProvider {
    project_id: String,
    region: String,
    access_token: String,
    model_id: String,
    client: Client,
}

impl VertexProvider {
    pub fn new(project_id: &str, region: &str, access_token: &str, model_id: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
            region: region.to_string(),
            access_token: access_token.to_string(),
            model_id: model_id.to_string(),
            client: Client::new(),
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Option<Self> {
        let project_id = std::env::var("GOOGLE_CLOUD_PROJECT")
            .or_else(|_| std::env::var("GCP_PROJECT_ID")).ok()?;
        let region = std::env::var("GOOGLE_CLOUD_REGION")
            .unwrap_or_else(|_| "us-central1".to_string());
        let access_token = std::env::var("GOOGLE_ACCESS_TOKEN")
            .or_else(|_| std::env::var("VERTEX_ACCESS_TOKEN")).ok()?;
        let model_id = std::env::var("VERTEX_MODEL")
            .unwrap_or_else(|_| "gemini-2.5-pro".to_string());

        Some(Self::new(&project_id, &region, &access_token, &model_id))
    }

    fn endpoint(&self) -> String {
        if self.model_id.contains("claude") {
            // Anthropic models via Vertex
            format!(
                "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/anthropic/models/{}:rawPredict",
                self.region, self.project_id, self.region, self.model_id
            )
        } else {
            // Gemini models
            format!(
                "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:generateContent",
                self.region, self.project_id, self.region, self.model_id
            )
        }
    }

    /// Build request body based on model type
    fn build_body(&self, request: &CreateMessageRequest) -> Value {
        if self.model_id.contains("claude") {
            // Anthropic format for Vertex
            let messages: Vec<Value> = request.messages.iter().filter_map(|m| {
                let role = match m.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::System => return None,
                };
                let content: Vec<Value> = m.content.iter().map(|b| match b {
                    ContentBlock::Text { text } => json!({"type": "text", "text": text}),
                    ContentBlock::ToolUse { id, name, input } => json!({
                        "type": "tool_use", "id": id, "name": name, "input": input
                    }),
                    ContentBlock::ToolResult { tool_use_id, content, is_error } => json!({
                        "type": "tool_result", "tool_use_id": tool_use_id,
                        "content": content, "is_error": is_error
                    }),
                }).collect();
                Some(json!({"role": role, "content": content}))
            }).collect();

            let mut body = json!({
                "anthropic_version": "vertex-2023-10-16",
                "messages": messages,
                "max_tokens": request.max_tokens,
            });

            if let Some(ref system) = request.system {
                body["system"] = json!(system);
            }

            if !request.tools.is_empty() {
                let tools: Vec<Value> = request.tools.iter().map(|t| json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                })).collect();
                body["tools"] = json!(tools);
            }

            body
        } else {
            // Gemini format
            let contents: Vec<Value> = request.messages.iter().filter_map(|m| {
                let role = match m.role {
                    Role::User => "user",
                    Role::Assistant => "model",
                    Role::System => return None,
                };
                let parts: Vec<Value> = m.content.iter().map(|b| match b {
                    ContentBlock::Text { text } => json!({"text": text}),
                    ContentBlock::ToolUse { name, input, .. } => json!({
                        "functionCall": {"name": name, "args": input}
                    }),
                    ContentBlock::ToolResult { content, .. } => json!({
                        "functionResponse": {"name": "", "response": {"result": content}}
                    }),
                }).collect();
                Some(json!({"role": role, "parts": parts}))
            }).collect();

            let mut body = json!({
                "contents": contents,
                "generationConfig": {
                    "maxOutputTokens": request.max_tokens,
                }
            });

            if let Some(ref system) = request.system {
                body["systemInstruction"] = json!({"parts": [{"text": system}]});
            }

            body
        }
    }
}

#[async_trait]
impl Provider for VertexProvider {
    fn name(&self) -> &str { "vertex" }

    fn default_model(&self) -> &str { &self.model_id }

    async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse, ProviderError> {
        let body = self.build_body(request);

        let resp = self.client.post(&self.endpoint())
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Content-Type", "application/json")
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

        // Parse response based on model type
        let (content_blocks, stop_reason, usage) = if self.model_id.contains("claude") {
            parse_anthropic_response(&resp_body)
        } else {
            parse_gemini_response(&resp_body)
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
        // Fall back to non-streaming for now
        let _ = tx.send(StreamEvent::MessageStart { model: self.model_id.clone() });
        let response = self.create_message(request).await?;

        for block in &response.message.content {
            if let ContentBlock::Text { text } = block {
                let _ = tx.send(StreamEvent::TextDelta { text: text.clone() });
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
            "gemini-2.5-pro".to_string(),
            "gemini-2.5-flash".to_string(),
            "claude-3-5-sonnet-v2@20241022".to_string(),
            "claude-3-haiku@20240307".to_string(),
        ])
    }
}

fn parse_anthropic_response(body: &Value) -> (Vec<ContentBlock>, StopReason, Usage) {
    let mut blocks = Vec::new();
    if let Some(content) = body["content"].as_array() {
        for block in content {
            if let Some(text) = block["text"].as_str() {
                blocks.push(ContentBlock::Text { text: text.to_string() });
            }
            if block["type"].as_str() == Some("tool_use") {
                blocks.push(ContentBlock::ToolUse {
                    id: block["id"].as_str().unwrap_or("").to_string(),
                    name: block["name"].as_str().unwrap_or("").to_string(),
                    input: block["input"].clone(),
                });
            }
        }
    }

    let stop_reason = match body["stop_reason"].as_str() {
        Some("tool_use") => StopReason::ToolUse,
        Some("max_tokens") => StopReason::MaxTokens,
        _ => StopReason::EndTurn,
    };

    let usage = Usage {
        input_tokens: body["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
        output_tokens: body["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
    };

    (blocks, stop_reason, usage)
}

fn parse_gemini_response(body: &Value) -> (Vec<ContentBlock>, StopReason, Usage) {
    let mut blocks = Vec::new();
    if let Some(candidates) = body["candidates"].as_array() {
        if let Some(first) = candidates.first() {
            if let Some(parts) = first["content"]["parts"].as_array() {
                for part in parts {
                    if let Some(text) = part["text"].as_str() {
                        blocks.push(ContentBlock::Text { text: text.to_string() });
                    }
                }
            }
        }
    }

    let usage = Usage {
        input_tokens: body["usageMetadata"]["promptTokenCount"].as_u64().unwrap_or(0) as u32,
        output_tokens: body["usageMetadata"]["candidatesTokenCount"].as_u64().unwrap_or(0) as u32,
    };

    (blocks, StopReason::EndTurn, usage)
}
