use super::transport::HttpTransport;
use super::{
    CreateMessageRequest, CreateMessageResponse, Provider, ProviderError, StopReason,
    StreamEvent,
};
use crate::message::{ContentBlock, Message, Role, Usage};
use async_trait::async_trait;
use futures::StreamExt;
use tokio::sync::mpsc;

/// OpenAI-compatible provider — works with OpenAI, DeepSeek, GLM, Grok, Ollama, etc.
pub struct OpenAiCompatProvider {
    transport: HttpTransport,
    provider_name: String,
    default_model: String,
}

impl OpenAiCompatProvider {
    pub fn new(
        provider_name: &str,
        api_key: &str,
        base_url: &str,
        model: Option<String>,
    ) -> Result<Self, ProviderError> {
        let default_model = model.unwrap_or_else(|| "gpt-4o".to_string());

        let headers = vec![("content-type", "application/json")];
        // Ollama doesn't need auth
        let transport = if api_key.is_empty() {
            HttpTransport::new(base_url, "", headers)?
        } else {
            HttpTransport::new(base_url, api_key, headers)?
        };

        Ok(Self {
            transport,
            provider_name: provider_name.to_string(),
            default_model,
        })
    }

    fn build_request_body(
        &self,
        request: &CreateMessageRequest,
        stream: bool,
    ) -> serde_json::Value {
        let mut messages = Vec::new();

        // System message
        if let Some(ref system) = request.system {
            messages.push(serde_json::json!({
                "role": "system",
                "content": system,
            }));
        }

        // Conversation messages
        for msg in &request.messages {
            let role_str = match msg.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => "system",
            };

            // Convert content blocks to OpenAI format
            let content_parts: Vec<serde_json::Value> = msg
                .content
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => {
                        Some(serde_json::json!({"type": "text", "text": text}))
                    }
                    _ => None,
                })
                .collect();

            // Check for tool calls in assistant messages
            let tool_calls: Vec<serde_json::Value> = msg
                .content
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::ToolUse { id, name, input } => {
                        Some(serde_json::json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": input.to_string(),
                            }
                        }))
                    }
                    _ => None,
                })
                .collect();

            // Check for tool results
            let tool_results: Vec<&ContentBlock> = msg
                .content
                .iter()
                .filter(|b| matches!(b, ContentBlock::ToolResult { .. }))
                .collect();

            if !tool_results.is_empty() {
                // Tool result messages
                for block in tool_results {
                    if let ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } = block
                    {
                        messages.push(serde_json::json!({
                            "role": "tool",
                            "tool_call_id": tool_use_id,
                            "content": content,
                        }));
                    }
                }
            } else if !tool_calls.is_empty() {
                // Assistant message with tool calls
                let simple_content = if content_parts.len() == 1 {
                    content_parts[0]["text"].as_str().map(|s| s.to_string())
                } else {
                    None
                };
                let mut msg_json = serde_json::json!({
                    "role": role_str,
                    "tool_calls": tool_calls,
                });
                if let Some(text) = simple_content {
                    msg_json["content"] = serde_json::json!(text);
                }
                messages.push(msg_json);
            } else if content_parts.len() == 1 {
                // Simple text message
                messages.push(serde_json::json!({
                    "role": role_str,
                    "content": content_parts[0]["text"],
                }));
            } else if !content_parts.is_empty() {
                messages.push(serde_json::json!({
                    "role": role_str,
                    "content": content_parts,
                }));
            }
        }

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_tokens,
        });

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if !request.tools.is_empty() {
            body["tools"] = serde_json::json!(request.tools.iter().map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }
                })
            }).collect::<Vec<_>>());
        }
        if stream {
            body["stream"] = serde_json::json!(true);
            body["stream_options"] = serde_json::json!({"include_usage": true});
        }

        body
    }

    fn parse_response(
        &self,
        value: &serde_json::Value,
    ) -> Result<CreateMessageResponse, ProviderError> {
        let model = value["model"].as_str().unwrap_or("unknown").to_string();
        let choice = &value["choices"][0];

        let stop_reason = match choice["finish_reason"].as_str() {
            Some("stop") => StopReason::EndTurn,
            Some("tool_calls") => StopReason::ToolUse,
            Some("length") => StopReason::MaxTokens,
            Some(other) => StopReason::Unknown(other.to_string()),
            None => StopReason::EndTurn,
        };

        let usage = Usage {
            input_tokens: value["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: value["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
        };

        let mut content = Vec::new();
        let msg = &choice["message"];

        if let Some(text) = msg["content"].as_str() {
            if !text.is_empty() {
                content.push(ContentBlock::text(text));
            }
        }

        if let Some(tool_calls) = msg["tool_calls"].as_array() {
            for tc in tool_calls {
                let id = tc["id"].as_str().unwrap_or("").to_string();
                let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
                let args_str = tc["function"]["arguments"].as_str().unwrap_or("{}");
                let input: serde_json::Value =
                    serde_json::from_str(args_str).unwrap_or_default();
                content.push(ContentBlock::ToolUse { id, name, input });
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
impl Provider for OpenAiCompatProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    fn default_model(&self) -> &str {
        &self.default_model
    }

    async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse, ProviderError> {
        let body = self.build_request_body(request, false);
        let resp = self.transport.post_json("/v1/chat/completions", &body).await?;
        self.parse_response(&resp)
    }

    async fn create_message_stream(
        &self,
        request: &CreateMessageRequest,
        tx: mpsc::UnboundedSender<StreamEvent>,
    ) -> Result<CreateMessageResponse, ProviderError> {
        let body = self.build_request_body(request, true);
        let resp = self.transport.post_stream("/v1/chat/completions", &body).await?;

        let mut final_content = Vec::new();
        let mut current_text = String::new();
        let mut current_tool_calls: Vec<(String, String, String)> = Vec::new(); // (id, name, args)
        let mut final_usage = Usage::default();
        let mut final_stop = StopReason::EndTurn;
        let mut final_model = String::new();

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| ProviderError::Stream(e.to_string()))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();

                let data = match line.strip_prefix("data: ") {
                    Some(d) => d.trim(),
                    None => continue,
                };

                if data == "[DONE]" {
                    let _ = tx.send(StreamEvent::MessageStop);
                    continue;
                }

                let parsed: serde_json::Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let choice = &parsed["choices"][0];
                let delta = &choice["delta"];

                if final_model.is_empty() {
                    if let Some(m) = parsed["model"].as_str() {
                        final_model = m.to_string();
                        let _ = tx.send(StreamEvent::MessageStart {
                            model: final_model.clone(),
                        });
                    }
                }

                // Text delta
                if let Some(text) = delta["content"].as_str() {
                    current_text.push_str(text);
                    let _ = tx.send(StreamEvent::TextDelta {
                        text: text.to_string(),
                    });
                }

                // Tool call deltas
                if let Some(tool_calls) = delta["tool_calls"].as_array() {
                    for tc in tool_calls {
                        let idx = tc["index"].as_u64().unwrap_or(0) as usize;
                        while current_tool_calls.len() <= idx {
                            current_tool_calls.push((String::new(), String::new(), String::new()));
                        }
                        if let Some(id) = tc["id"].as_str() {
                            if !id.is_empty() {
                                current_tool_calls[idx].0 = id.to_string();
                            }
                        }
                        if let Some(name) = tc["function"]["name"].as_str() {
                            if !name.is_empty() {
                                let is_new = current_tool_calls[idx].1.is_empty();
                                current_tool_calls[idx].1.push_str(name);
                                if is_new {
                                    let _ = tx.send(StreamEvent::ToolUseStart {
                                        id: current_tool_calls[idx].0.clone(),
                                        name: current_tool_calls[idx].1.clone(),
                                    });
                                }
                            }
                        }
                        if let Some(args) = tc["function"]["arguments"].as_str() {
                            if !args.is_empty() {
                                current_tool_calls[idx].2.push_str(args);
                                let _ = tx.send(StreamEvent::ToolInputDelta {
                                    partial_json: args.to_string(),
                                });
                            }
                        }
                    }
                }

                // Finish reason
                if let Some(reason) = choice["finish_reason"].as_str() {
                    final_stop = match reason {
                        "stop" => StopReason::EndTurn,
                        "tool_calls" => StopReason::ToolUse,
                        "length" => StopReason::MaxTokens,
                        other => StopReason::Unknown(other.to_string()),
                    };
                }

                // Usage (some providers include it in stream)
                if let Some(usage) = parsed.get("usage") {
                    final_usage.input_tokens =
                        usage["prompt_tokens"].as_u64().unwrap_or(0) as u32;
                    final_usage.output_tokens =
                        usage["completion_tokens"].as_u64().unwrap_or(0) as u32;
                }
            }
        }

        // Build final content
        if !current_text.is_empty() {
            final_content.push(ContentBlock::text(&current_text));
        }
        for (id, name, args) in &current_tool_calls {
            let input: serde_json::Value =
                serde_json::from_str(args).unwrap_or_default();
            final_content.push(ContentBlock::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input,
            });
        }

        let _ = tx.send(StreamEvent::MessageDelta {
            stop_reason: final_stop.clone(),
            usage: final_usage.clone(),
        });

        Ok(CreateMessageResponse {
            message: Message {
                role: Role::Assistant,
                content: final_content,
            },
            usage: final_usage,
            stop_reason: final_stop,
            model: final_model,
        })
    }

    /// Fetch available models from GET /v1/models
    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        let resp = self.transport.get_json("/v1/models").await?;

        let mut models: Vec<String> = Vec::new();

        if let Some(data) = resp.get("data").and_then(|d| d.as_array()) {
            for item in data {
                if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                    models.push(id.to_string());
                }
            }
        }

        // Sort alphabetically for nice display
        models.sort();

        Ok(models)
    }
}
