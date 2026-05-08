//! AWS Bedrock Provider
//! Uses AWS Bedrock's converse API endpoint with SigV4 auth.
//! Falls back to bedrock-runtime invoke-model for streaming.

use super::*;
use crate::message::{ContentBlock, Message, Role, Usage};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
pub struct BedrockProvider {
    provider_name: String,
    region: String,
    access_key: String,
    secret_key: String,
    session_token: Option<String>,
    model_id: String,
    client: Client,
}

impl BedrockProvider {
    pub fn new(region: &str, access_key: &str, secret_key: &str, model_id: &str) -> Self {
        Self::new_named("bedrock", region, access_key, secret_key, model_id)
    }

    pub fn new_named(
        provider_name: &str,
        region: &str,
        access_key: &str,
        secret_key: &str,
        model_id: &str,
    ) -> Self {
        Self::new_with_session_named(
            provider_name,
            region,
            access_key,
            secret_key,
            None,
            model_id,
        )
    }

    pub fn new_with_session(
        region: &str,
        access_key: &str,
        secret_key: &str,
        session_token: Option<String>,
        model_id: &str,
    ) -> Self {
        Self::new_with_session_named(
            "bedrock",
            region,
            access_key,
            secret_key,
            session_token,
            model_id,
        )
    }

    pub fn new_with_session_named(
        provider_name: &str,
        region: &str,
        access_key: &str,
        secret_key: &str,
        session_token: Option<String>,
        model_id: &str,
    ) -> Self {
        Self {
            provider_name: provider_name.to_string(),
            region: region.to_string(),
            access_key: access_key.to_string(),
            secret_key: secret_key.to_string(),
            session_token,
            model_id: model_id.to_string(),
            client: Client::new(),
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Option<Self> {
        Self::from_env_with_model(None)
    }

    pub fn from_env_with_model(model_id: Option<String>) -> Option<Self> {
        Self::from_env_with_model_named("bedrock", model_id)
    }

    pub fn from_env_with_model_named(
        provider_name: &str,
        model_id: Option<String>,
    ) -> Option<Self> {
        let region = std::env::var("AWS_REGION")
            .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
            .unwrap_or_else(|_| "us-east-1".to_string());
        let access_key = std::env::var("AWS_ACCESS_KEY_ID").ok()?;
        let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").ok()?;
        let session_token = std::env::var("AWS_SESSION_TOKEN").ok();
        let model_id = model_id
            .or_else(|| std::env::var("AWS_BEDROCK_MODEL").ok())
            .unwrap_or_else(|| "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string());

        Some(Self::new_with_session_named(
            provider_name,
            &region,
            &access_key,
            &secret_key,
            session_token,
            &model_id,
        ))
    }

    fn host(&self) -> String {
        format!("bedrock-runtime.{}.amazonaws.com", self.region)
    }

    fn canonical_uri(&self) -> String {
        format!("/model/{}/converse", encode_path_segment(&self.model_id))
    }

    fn endpoint(&self) -> String {
        format!("https://{}{}", self.host(), self.canonical_uri())
    }

    #[allow(dead_code)]
    fn stream_endpoint(&self) -> String {
        format!(
            "https://{}/model/{}/converse-stream",
            self.host(),
            encode_path_segment(&self.model_id)
        )
    }

    /// Build the Bedrock converse request body
    fn build_body(&self, request: &CreateMessageRequest) -> Value {
        let messages: Vec<Value> =
            request
                .messages
                .iter()
                .filter_map(|m| {
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
                ContentBlock::Image { source } => json!({
                    "image": {
                        "format": source.media_type.strip_prefix("image/").unwrap_or("png"),
                        "source": { "bytes": source.data }
                    }
                }),
                ContentBlock::Thinking { text } => json!({"text": format!("[thinking] {text}")}),
            }).collect();
                    Some(json!({"role": role, "content": content}))
                })
                .collect();

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
            let tool_config: Vec<Value> = request
                .tools
                .iter()
                .map(|t| {
                    json!({
                        "toolSpec": {
                            "name": t.name,
                            "description": t.description,
                            "inputSchema": {"json": t.input_schema}
                        }
                    })
                })
                .collect();
            body["toolConfig"] = json!({"tools": tool_config});
        }

        body
    }

    fn sign_request(&self, payload: &str) -> SignedRequest {
        let now = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();
        let payload_hash = sha256_hex(payload.as_bytes());
        let host = self.host();
        let canonical_uri = self.canonical_uri();

        let mut headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("host".to_string(), host.clone()),
            ("x-amz-content-sha256".to_string(), payload_hash.clone()),
            ("x-amz-date".to_string(), amz_date.clone()),
        ];
        if let Some(token) = &self.session_token {
            headers.push(("x-amz-security-token".to_string(), token.clone()));
        }
        headers.sort_by(|a, b| a.0.cmp(&b.0));

        let canonical_headers = headers
            .iter()
            .map(|(name, value)| format!("{name}:{}\n", value.trim()))
            .collect::<String>();
        let signed_headers = headers
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(";");

        let canonical_request =
            format!("POST\n{canonical_uri}\n\n{canonical_headers}{signed_headers}\n{payload_hash}");
        let credential_scope = format!("{date_stamp}/{}/bedrock/aws4_request", self.region);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
            sha256_hex(canonical_request.as_bytes())
        );

        let signing_key = signing_key(&self.secret_key, &date_stamp, &self.region, "bedrock");
        let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));
        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.access_key
        );

        SignedRequest {
            host,
            url: self.endpoint(),
            amz_date,
            payload_hash,
            authorization,
            session_token: self.session_token.clone(),
        }
    }
}

#[async_trait]
impl Provider for BedrockProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    fn default_model(&self) -> &str {
        &self.model_id
    }

    async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse, ProviderError> {
        let body = self.build_body(request);
        let payload =
            serde_json::to_string(&body).map_err(|e| ProviderError::Deserialize(e.to_string()))?;
        let signed = self.sign_request(&payload);

        let mut req = self
            .client
            .post(&signed.url)
            .header("Content-Type", "application/json")
            .header("Host", &signed.host)
            .header("X-Amz-Date", &signed.amz_date)
            .header("X-Amz-Content-Sha256", &signed.payload_hash)
            .header("Authorization", &signed.authorization)
            .body(payload);
        if let Some(token) = &signed.session_token {
            req = req.header("X-Amz-Security-Token", token);
        }
        let resp = req.send().await?;

        let status = resp.status().as_u16();
        if status != 200 {
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api {
                status,
                message: text,
            });
        }

        let resp_body: Value = resp
            .json()
            .await
            .map_err(|e| ProviderError::Deserialize(e.to_string()))?;

        // Parse Bedrock converse response
        let output = &resp_body["output"]["message"];
        let mut content_blocks = Vec::new();

        if let Some(content) = output["content"].as_array() {
            for block in content {
                if let Some(text) = block["text"].as_str() {
                    content_blocks.push(ContentBlock::Text {
                        text: text.to_string(),
                    });
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
        let _ = tx.send(StreamEvent::MessageStart {
            model: self.model_id.clone(),
        });
        let response = self.create_message(request).await?;

        for block in &response.message.content {
            match block {
                ContentBlock::Text { text } => {
                    let _ = tx.send(StreamEvent::TextDelta { text: text.clone() });
                }
                ContentBlock::ToolUse { id, name, input } => {
                    let _ = tx.send(StreamEvent::ToolUseStart {
                        id: id.clone(),
                        name: name.clone(),
                    });
                    let _ = tx.send(StreamEvent::ToolInputDelta {
                        partial_json: input.to_string(),
                    });
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

struct SignedRequest {
    host: String,
    url: String,
    amz_date: String,
    payload_hash: String,
    authorization: String,
    session_token: Option<String>,
}

fn encode_path_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.as_bytes() {
        let ch = *byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~') {
            out.push(ch);
        } else {
            out.push('%');
            out.push_str(&format!("{byte:02X}"));
        }
    }
    out
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    const BLOCK_SIZE: usize = 64;

    let mut key_block = vec![0u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let mut hasher = Sha256::new();
        hasher.update(key);
        let digest = hasher.finalize();
        key_block[..digest.len()].copy_from_slice(&digest);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut o_key_pad = vec![0u8; BLOCK_SIZE];
    let mut i_key_pad = vec![0u8; BLOCK_SIZE];
    for (idx, byte) in key_block.iter().enumerate() {
        o_key_pad[idx] = byte ^ 0x5c;
        i_key_pad[idx] = byte ^ 0x36;
    }

    let mut inner = Sha256::new();
    inner.update(&i_key_pad);
    inner.update(data);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(&o_key_pad);
    outer.update(inner_hash);
    outer.finalize().to_vec()
}

fn signing_key(secret: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac_sha256(format!("AWS4{secret}").as_bytes(), date.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

#[cfg(test)]
mod tests {
    use super::BedrockProvider;

    #[test]
    fn canonical_uri_encodes_model_id() {
        let provider = BedrockProvider::new(
            "us-east-1",
            "akid",
            "secret",
            "anthropic.claude-3-5-sonnet-20241022-v2:0",
        );

        assert_eq!(
            provider.canonical_uri(),
            "/model/anthropic.claude-3-5-sonnet-20241022-v2%3A0/converse"
        );
    }
}
