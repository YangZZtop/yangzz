use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{Value, json};

pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web using DuckDuckGo. Returns titles, URLs, and snippets of top results. Use for finding documentation, current information, or solutions to problems."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return (default: 8, max: 20)",
                    "default": 8
                }
            },
            "required": ["query"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: &Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let query = input["query"]
            .as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'query' field".into()))?;

        let max_results = input["max_results"].as_u64().unwrap_or(8).min(20) as usize;

        let results = search_ddg(query, max_results).await?;

        if results.is_empty() {
            return Ok(ToolOutput::success(format!(
                "No results found for: {query}"
            )));
        }

        let mut output = format!("Search results for: {query}\n\n");
        for (i, result) in results.iter().enumerate() {
            output.push_str(&format!(
                "{}. {}\n   {}\n   {}\n\n",
                i + 1,
                result.title,
                result.url,
                result.snippet
            ));
        }

        Ok(ToolOutput::success(output))
    }
}

struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

/// Search DuckDuckGo HTML version (no API key needed)
async fn search_ddg(query: &str, max_results: usize) -> Result<Vec<SearchResult>, ToolError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| ToolError::Execution(format!("HTTP client error: {e}")))?;

    let url = format!("https://html.duckduckgo.com/html/?q={}", urlencoded(query));

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| ToolError::Execution(format!("Search request failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(ToolError::Execution(format!(
            "Search returned HTTP {}",
            resp.status()
        )));
    }

    let body = resp
        .text()
        .await
        .map_err(|e| ToolError::Execution(format!("Cannot read response: {e}")))?;

    Ok(parse_ddg_html(&body, max_results))
}

/// Parse DuckDuckGo HTML search results
fn parse_ddg_html(html: &str, max_results: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();

    // DuckDuckGo HTML results are in <div class="result"> blocks
    // Each has: <a class="result__a" href="...">title</a>
    //           <a class="result__snippet">snippet</a>
    //           <a class="result__url" href="...">url</a>

    // Simple regex-based extraction (no HTML parser dependency needed)
    let result_re = regex::Regex::new(
        r#"class="result__a"[^>]*href="([^"]*)"[^>]*>([^<]*)</a>"#
    ).ok();
    let snippet_re = regex::Regex::new(
        r#"class="result__snippet"[^>]*>([^<]*(?:<[^>]*>[^<]*)*)</a>"#
    ).ok();

    if let Some(ref re) = result_re {
        let snippet_regex = snippet_re.as_ref();
        let mut snippet_iter = snippet_regex.map(|r| r.captures_iter(html));

        for cap in re.captures_iter(html) {
            if results.len() >= max_results {
                break;
            }

            let raw_url = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let title = cap.get(2).map(|m| m.as_str()).unwrap_or("").trim();

            // DuckDuckGo wraps URLs in a redirect — extract the actual URL
            let url = extract_ddg_url(raw_url);

            // Skip empty/invalid results
            if title.is_empty() || url.is_empty() {
                continue;
            }

            // Try to get the corresponding snippet
            let snippet = snippet_iter
                .as_mut()
                .and_then(|iter| iter.next())
                .and_then(|c| c.get(1))
                .map(|m| strip_html_tags(m.as_str()))
                .unwrap_or_default();

            results.push(SearchResult {
                title: html_decode(title),
                url,
                snippet: html_decode(&snippet),
            });
        }
    }

    results
}

/// Extract actual URL from DuckDuckGo redirect URL
fn extract_ddg_url(raw: &str) -> String {
    // DDG format: //duckduckgo.com/l/?uddg=https%3A%2F%2F...&rut=...
    if let Some(pos) = raw.find("uddg=") {
        let encoded = &raw[pos + 5..];
        let end = encoded.find('&').unwrap_or(encoded.len());
        let encoded = &encoded[..end];
        return urldecoded(encoded);
    }
    // Direct URL
    if raw.starts_with("http") {
        return raw.to_string();
    }
    if raw.starts_with("//") {
        return format!("https:{raw}");
    }
    raw.to_string()
}

/// Simple URL encoding
fn urlencoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push_str(&format!("{byte:02X}"));
            }
        }
    }
    out
}

/// Simple URL decoding
fn urldecoded(s: &str) -> String {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Strip HTML tags from a string
fn strip_html_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(ch);
        }
    }
    out.trim().to_string()
}

/// Decode common HTML entities
fn html_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}
