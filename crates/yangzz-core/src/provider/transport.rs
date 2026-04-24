use super::ProviderError;
use reqwest::{Client, header};

/// Shared HTTP transport for all providers
#[derive(Clone)]
pub struct HttpTransport {
    client: Client,
    base_url: String,
}

impl HttpTransport {
    pub fn new(
        base_url: impl Into<String>,
        api_key: &str,
        extra_headers: Vec<(&str, &str)>,
    ) -> Result<Self, ProviderError> {
        let mut headers = header::HeaderMap::new();
        for (k, v) in extra_headers {
            headers.insert(
                header::HeaderName::from_bytes(k.as_bytes())
                    .map_err(|e| ProviderError::Other(e.to_string()))?,
                header::HeaderValue::from_str(v)
                    .map_err(|e| ProviderError::Other(e.to_string()))?,
            );
        }

        if !api_key.is_empty() {
            headers.insert(
                header::AUTHORIZATION,
                header::HeaderValue::from_str(&format!("Bearer {api_key}"))
                    .map_err(|e| ProviderError::Other(e.to_string()))?,
            );
        }

        let client = Client::builder().default_headers(headers).build()?;

        // Normalize base_url: trim trailing slash so that `base_url + "/v1/..."`
        // never produces a double slash. Path prefix (e.g. "/antigravity") is preserved.
        let base_url = base_url.into().trim_end_matches('/').to_string();

        Ok(Self { client, base_url })
    }

    /// POST JSON and get full response. Automatically retries once on 429 /
    /// transient errors, respecting `Retry-After` if present.
    pub async fn post_json(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ProviderError> {
        let url = format!("{}{}", self.base_url, path);

        // First attempt
        let resp = self.client.post(&url).json(body).send().await?;
        let status = resp.status().as_u16();

        // 429 → honor Retry-After and retry once
        if status == 429 {
            let wait = parse_retry_after(&resp).unwrap_or(2);
            let wait = wait.clamp(1, 30);
            tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
            let resp2 = self.client.post(&url).json(body).send().await?;
            return handle_json_response(resp2).await;
        }

        handle_json_response(resp).await
    }

    /// GET JSON response
    pub async fn get_json(&self, path: &str) -> Result<serde_json::Value, ProviderError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.get(&url).send().await?;
        let status = resp.status().as_u16();

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api {
                status,
                message: text,
            });
        }

        resp.json()
            .await
            .map_err(|e| ProviderError::Deserialize(e.to_string()))
    }

    /// POST JSON and get SSE stream. Retries once on 429 with backoff.
    pub async fn post_stream(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<reqwest::Response, ProviderError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.post(&url).json(body).send().await?;
        let status = resp.status().as_u16();

        if status == 429 {
            let wait = parse_retry_after(&resp).unwrap_or(2);
            let wait = wait.clamp(1, 30);
            tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
            let resp2 = self.client.post(&url).json(body).send().await?;
            return handle_stream_response(resp2).await;
        }

        handle_stream_response(resp).await
    }
}

/// Centralized response → ProviderError translation with user-friendly messages.
async fn handle_json_response(resp: reqwest::Response) -> Result<serde_json::Value, ProviderError> {
    let status = resp.status().as_u16();
    if resp.status().is_success() {
        return resp
            .json()
            .await
            .map_err(|e| ProviderError::Deserialize(e.to_string()));
    }
    let body = resp.text().await.unwrap_or_default();
    Err(translate_status_error(status, &body))
}

async fn handle_stream_response(
    resp: reqwest::Response,
) -> Result<reqwest::Response, ProviderError> {
    let status = resp.status().as_u16();
    if resp.status().is_success() {
        return Ok(resp);
    }
    let body = resp.text().await.unwrap_or_default();
    Err(translate_status_error(status, &body))
}

/// Turn a status code + body into a friendly ProviderError. Replaces raw
/// HTML error pages and cryptic upstream messages with actionable hints.
fn translate_status_error(status: u16, body: &str) -> ProviderError {
    // Extract server-provided message if JSON; otherwise truncate HTML
    let cleaned = extract_server_message(body);

    match status {
        401 | 403 => ProviderError::Auth(format!(
            "API key 被拒绝（HTTP {status}）。\n\
             → 检查 /key <provider>，或 /provider edit <name> 更新\n\
             → 服务端信息: {cleaned}"
        )),
        404 => ProviderError::Api {
            status,
            message: format!(
                "端点/模型不存在（HTTP 404）。\n\
                 → 多半是模型名错了，试试 /model 看当前 provider 支持什么\n\
                 → 或检查 base_url 是否正确（是否漏掉了 /v1 路径前缀？）\n\
                 → 服务端信息: {cleaned}"
            ),
        },
        400 => ProviderError::Api {
            status,
            message: format!(
                "请求格式错误（HTTP 400）。\n\
                 → 常见原因：模型名不被该 provider 支持 / 参数超限\n\
                 → 服务端信息: {cleaned}"
            ),
        },
        429 => ProviderError::RateLimit {
            retry_after_secs: None,
        },
        500..=599 => ProviderError::Api {
            status,
            message: format!(
                "服务端错误（HTTP {status}）— 通常是 provider 一时抽风，稍后重试。\n\
                 → 服务端信息: {cleaned}"
            ),
        },
        _ => ProviderError::Api {
            status,
            message: format!("HTTP {status}: {cleaned}"),
        },
    }
}

fn extract_server_message(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "(empty body)".into();
    }
    // Try JSON first — OpenAI/Anthropic style errors have {"error": {"message": ...}}
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(msg) = v
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
        {
            return msg.to_string();
        }
        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
            return msg.to_string();
        }
    }
    // Avoid dumping HTML
    if trimmed.starts_with('<') {
        return "(HTML error page — check base_url and network)".into();
    }
    // Cap length
    let short: String = trimmed.chars().take(200).collect();
    if trimmed.len() > short.len() {
        format!("{short}…")
    } else {
        short
    }
}

fn parse_retry_after(resp: &reqwest::Response) -> Option<u64> {
    resp.headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
}
