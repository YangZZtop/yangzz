use reqwest::{Client, header};
use super::ProviderError;

/// Shared HTTP transport for all providers
#[derive(Clone)]
pub struct HttpTransport {
    client: Client,
    base_url: String,
}

impl HttpTransport {
    pub fn new(base_url: impl Into<String>, api_key: &str, extra_headers: Vec<(&str, &str)>) -> Result<Self, ProviderError> {
        let mut headers = header::HeaderMap::new();
        for (k, v) in extra_headers {
            headers.insert(
                header::HeaderName::from_bytes(k.as_bytes()).map_err(|e| ProviderError::Other(e.to_string()))?,
                header::HeaderValue::from_str(v).map_err(|e| ProviderError::Other(e.to_string()))?,
            );
        }

        if !api_key.is_empty() {
            headers.insert(
                header::AUTHORIZATION,
                header::HeaderValue::from_str(&format!("Bearer {api_key}"))
                    .map_err(|e| ProviderError::Other(e.to_string()))?,
            );
        }

        let client = Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            client,
            base_url: base_url.into(),
        })
    }

    /// POST JSON and get full response
    pub async fn post_json(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ProviderError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.post(&url).json(body).send().await?;
        let status = resp.status().as_u16();

        if status == 401 || status == 403 {
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Auth(text));
        }
        if status == 429 {
            return Err(ProviderError::RateLimit { retry_after_secs: None });
        }
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api { status, message: text });
        }

        resp.json().await.map_err(|e| ProviderError::Deserialize(e.to_string()))
    }

    /// GET JSON response
    pub async fn get_json(
        &self,
        path: &str,
    ) -> Result<serde_json::Value, ProviderError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.get(&url).send().await?;
        let status = resp.status().as_u16();

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api { status, message: text });
        }

        resp.json().await.map_err(|e| ProviderError::Deserialize(e.to_string()))
    }

    /// POST JSON and get SSE stream
    pub async fn post_stream(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<reqwest::Response, ProviderError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.post(&url).json(body).send().await?;
        let status = resp.status().as_u16();

        if status == 401 || status == 403 {
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Auth(text));
        }
        if status == 429 {
            return Err(ProviderError::RateLimit { retry_after_secs: None });
        }
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api { status, message: text });
        }

        Ok(resp)
    }
}
