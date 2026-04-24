use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{Value, json};

pub struct FetchTool;

#[async_trait]
impl Tool for FetchTool {
    fn name(&self) -> &str {
        "fetch"
    }

    fn description(&self) -> &str {
        "Fetch content from a URL. Returns the text content of the response."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "URL to fetch" },
                "method": { "type": "string", "description": "HTTP method (default: GET)", "default": "GET" }
            },
            "required": ["url"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: &Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let url = input["url"]
            .as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'url'".into()))?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ToolError::Execution(format!("HTTP client error: {e}")))?;

        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Fetch failed: {e}")))?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| ToolError::Execution(format!("Cannot read body: {e}")))?;

        let mut result = body;
        if result.len() > 50000 {
            result.truncate(50000);
            result.push_str("\n... (truncated)");
        }

        if status.is_success() {
            Ok(ToolOutput::success(result))
        } else {
            Ok(ToolOutput::error(format!("HTTP {status}\n{result}")))
        }
    }
}
