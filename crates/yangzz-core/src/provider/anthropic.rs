use super::transport::HttpTransport;
use super::{
    CreateMessageRequest, CreateMessageResponse, Provider, ProviderError, StopReason,
    StreamEvent,
};
use crate::message::{ContentBlock, Message, Role, Usage};
use async_trait::async_trait;
use futures::StreamExt;
use tokio::sync::mpsc;

const API_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    transport: HttpTransport,
    default_model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: &str, model: Option<String>) -> Result<Self, ProviderError> {
        let transport = HttpTransport::new(
            "https://api.anthropic.com",
            "",
            vec![
                ("x-api-key", api_key),
                ("anthropic-version", API_VERSION),
                ("content-type", "application/json"),
            ],
        )?;

        Ok(Self {
            transport,
            default_model: model.unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
        })
    }

    pub fn with_base_url(api_key: &str, base_url: &str, model: Option<String>) -> Result<Self, ProviderError> {
        let transport = HttpTransport::new(
            base_url,
            "",
            vec![
                ("x-api-key", api_key),
                ("anthropic-version", API_VERSION),
                ("content-type", "application/json"),
            ],
        )?;

        Ok(Self {
            transport,
            default_model: model.unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
        })
    }

    fn build_request_body(&self, request: &CreateMessageRequest, stream: bool) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": request.model,
            "max_tokens": request.max_tokens,
            "messages": request.messages.iter().map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                })
            }).collect::<Vec<_>>(),
        });

        if let Some(ref system) = request.system {
            body["system"] = serde_json::json!(system);
        }
        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if !request.tools.is_empty() {
            body["tools"] = serde_json::json!(request.tools.iter().map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                })
            }).collect::<Vec<_>>());
        }
        if stream {
            body["stream"] = serde_json::json!(true);
        }

        body
    }

    fn parse_response(&self, value: &serde_json::Value) -> Result<CreateMessageResponse, ProviderError> {
        let model = value["model"].as_str().unwrap_or("unknown").to_string();
        let stop_reason = match value["stop_reason"].as_str() {
            Some("end_turn") => StopReason::EndTurn,
            Some("tool_use") => StopReason::ToolUse,
            Some("max_tokens") => StopReason::MaxTokens,
            Some(other) => StopReason::Unknown(other.to_string()),
            None => StopReason::EndTurn,
        };
        let usage = Usage {
            input_tokens: value["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: value["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
        };

        let mut content = Vec::new();
        if let Some(blocks) = value["content"].as_array() {
            for block in blocks {
                match block["type"].as_str() {
                    Some("text") => {
                        content.push(ContentBlock::Text {
                            text: block["text"].as_str().unwrap_or("").to_string(),
                        });
                    }
                    Some("tool_use") => {
                        content.push(ContentBlock::ToolUse {
                            id: block["id"].as_str().unwrap_or("").to_string(),
                            name: block["name"].as_str().unwrap_or("").to_string(),
                            input: block["input"].clone(),
                        });
                    }
                    _ => {}
                }
            }
        }

        Ok(CreateMessageResponse {
            message: Message {
                role: Role::Assistant,
                content,
            },
            usage,
            stop_reason,
            model,
        })
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn default_model(&self) -> &str {
        &self.default_model
    }

    async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse, ProviderError> {
        let body = self.build_request_body(request, false);
        let resp = self.transport.post_json("/v1/messages", &body).await?;
        self.parse_response(&resp)
    }

    async fn create_message_stream(
        &self,
        request: &CreateMessageRequest,
        tx: mpsc::UnboundedSender<StreamEvent>,
    ) -> Result<CreateMessageResponse, ProviderError> {
        let body = self.build_request_body(request, true);
        let resp = self.transport.post_stream("/v1/messages", &body).await?;

        let mut final_message = Message {
            role: Role::Assistant,
            content: Vec::new(),
        };
        let mut final_usage = Usage::default();
        let mut final_stop = StopReason::EndTurn;
        let mut final_model = String::new();

        // Current block being built
        let mut current_text = String::new();
        let mut current_tool_id = String::new();
        let mut current_tool_name = String::new();
        let mut current_tool_input = String::new();
        let mut in_tool_use = false;

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| ProviderError::Stream(e.to_string()))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Parse SSE events from buffer
            while let Some(pos) = buffer.find("\n\n") {
                let event_text = buffer[..pos].to_string();
                buffer = buffer[pos + 2..].to_string();

                let mut event_type = "";
                let mut data = "";

                for line in event_text.lines() {
                    if let Some(rest) = line.strip_prefix("event: ") {
                        event_type = rest.trim();
                    } else if let Some(rest) = line.strip_prefix("data: ") {
                        data = rest.trim();
                    }
                }

                if data.is_empty() { continue; }

                let parsed: serde_json::Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                match event_type {
                    "message_start" => {
                        final_model = parsed["message"]["model"]
                            .as_str().unwrap_or("").to_string();
                        let _ = tx.send(StreamEvent::MessageStart {
                            model: final_model.clone(),
                        });
                    }
                    "content_block_start" => {
                        let idx = parsed["index"].as_u64().unwrap_or(0) as usize;
                        let block_type = parsed["content_block"]["type"].as_str().unwrap_or("");
                        if block_type == "tool_use" {
                            in_tool_use = true;
                            current_tool_id = parsed["content_block"]["id"]
                                .as_str().unwrap_or("").to_string();
                            current_tool_name = parsed["content_block"]["name"]
                                .as_str().unwrap_or("").to_string();
                            current_tool_input.clear();
                            let _ = tx.send(StreamEvent::ToolUseStart {
                                id: current_tool_id.clone(),
                                name: current_tool_name.clone(),
                            });
                        } else {
                            in_tool_use = false;
                            current_text.clear();
                        }
                        let _ = tx.send(StreamEvent::ContentBlockStart { index: idx });
                    }
                    "content_block_delta" => {
                        let delta_type = parsed["delta"]["type"].as_str().unwrap_or("");
                        if delta_type == "text_delta" {
                            let text = parsed["delta"]["text"]
                                .as_str().unwrap_or("").to_string();
                            current_text.push_str(&text);
                            let _ = tx.send(StreamEvent::TextDelta { text });
                        } else if delta_type == "input_json_delta" {
                            let json = parsed["delta"]["partial_json"]
                                .as_str().unwrap_or("").to_string();
                            current_tool_input.push_str(&json);
                            let _ = tx.send(StreamEvent::ToolInputDelta { partial_json: json });
                        }
                    }
                    "content_block_stop" => {
                        let idx = parsed["index"].as_u64().unwrap_or(0) as usize;
                        if in_tool_use {
                            let input: serde_json::Value =
                                serde_json::from_str(&current_tool_input)
                                    .unwrap_or(serde_json::Value::Object(Default::default()));
                            final_message.content.push(ContentBlock::ToolUse {
                                id: current_tool_id.clone(),
                                name: current_tool_name.clone(),
                                input,
                            });
                            in_tool_use = false;
                        } else {
                            if !current_text.is_empty() {
                                final_message.content.push(ContentBlock::Text {
                                    text: current_text.clone(),
                                });
                            }
                        }
                        let _ = tx.send(StreamEvent::ContentBlockStop { index: idx });
                    }
                    "message_delta" => {
                        final_stop = match parsed["delta"]["stop_reason"].as_str() {
                            Some("end_turn") => StopReason::EndTurn,
                            Some("tool_use") => StopReason::ToolUse,
                            Some("max_tokens") => StopReason::MaxTokens,
                            Some(other) => StopReason::Unknown(other.to_string()),
                            None => StopReason::EndTurn,
                        };
                        final_usage.output_tokens =
                            parsed["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32;
                        let _ = tx.send(StreamEvent::MessageDelta {
                            stop_reason: final_stop.clone(),
                            usage: final_usage.clone(),
                        });
                    }
                    "message_stop" => {
                        let _ = tx.send(StreamEvent::MessageStop);
                    }
                    "error" => {
                        let msg = parsed["error"]["message"]
                            .as_str().unwrap_or("unknown error").to_string();
                        let _ = tx.send(StreamEvent::Error { message: msg });
                    }
                    _ => {}
                }
            }
        }

        Ok(CreateMessageResponse {
            message: final_message,
            usage: final_usage,
            stop_reason: final_stop,
            model: final_model,
        })
    }
}
