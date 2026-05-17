//! Browser tool: fetch a URL and extract readable text content.
//! Unlike the raw `fetch` tool, this strips HTML/JS/CSS and returns
//! clean text suitable for the model to read — like a headless browser
//! "reader mode".

use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{Value, json};

pub struct BrowserTool;

#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser"
    }

    fn description(&self) -> &str {
        "Open a URL and extract readable text content (like browser reader mode). \
         Strips HTML/JS/CSS and returns clean text. Use for reading documentation, \
         articles, or any web page content."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to open and read"
                },
                "selector": {
                    "type": "string",
                    "description": "Optional: CSS-like content hint (e.g. 'main', 'article', '.content'). If provided, tries to extract only that section."
                }
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
            .ok_or_else(|| ToolError::Validation("Missing 'url' field".into()))?;

        let selector = input["selector"].as_str().unwrap_or("");

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| ToolError::Execution(format!("HTTP client error: {e}")))?;

        let resp = client
            .get(url)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Fetch failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Ok(ToolOutput::error(format!(
                "HTTP {status} for {url}"
            )));
        }

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let body = resp
            .text()
            .await
            .map_err(|e| ToolError::Execution(format!("Cannot read body: {e}")))?;

        // If it's not HTML, return raw (might be JSON, plain text, etc.)
        if !content_type.contains("html") {
            let mut result = body;
            if result.len() > 30000 {
                result.truncate(30000);
                result.push_str("\n... (truncated)");
            }
            return Ok(ToolOutput::success(format!("URL: {url}\nType: {content_type}\n\n{result}")));
        }

        // Extract readable text from HTML
        let text = extract_readable_text(&body, selector);

        if text.trim().is_empty() {
            return Ok(ToolOutput::success(format!(
                "URL: {url}\n(Page loaded but no readable text content extracted. Try using `fetch` for raw HTML.)"
            )));
        }

        let mut result = format!("URL: {url}\n\n{text}");
        if result.len() > 40000 {
            result.truncate(40000);
            result.push_str("\n... (truncated)");
        }

        Ok(ToolOutput::success(result))
    }
}

/// Extract readable text from HTML, stripping tags, scripts, styles.
/// If `selector_hint` is provided, try to focus on that section.
fn extract_readable_text(html: &str, selector_hint: &str) -> String {
    // Step 1: Remove script and style blocks entirely
    let cleaned = remove_blocks(html, "script");
    let cleaned = remove_blocks(&cleaned, "style");
    let cleaned = remove_blocks(&cleaned, "nav");
    let cleaned = remove_blocks(&cleaned, "footer");
    let cleaned = remove_blocks(&cleaned, "header");

    // Step 2: If selector hint provided, try to extract that section
    let focused = if !selector_hint.is_empty() {
        extract_section(&cleaned, selector_hint).unwrap_or(cleaned.clone())
    } else {
        // Try common content containers
        extract_section(&cleaned, "main")
            .or_else(|| extract_section(&cleaned, "article"))
            .or_else(|| extract_section(&cleaned, "role=\"main\""))
            .or_else(|| extract_section(&cleaned, "class=\"content\""))
            .or_else(|| extract_section(&cleaned, "id=\"content\""))
            .unwrap_or(cleaned)
    };

    // Step 3: Strip remaining HTML tags and normalize whitespace
    let text = strip_tags(&focused);

    // Step 4: Clean up whitespace
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    // Deduplicate consecutive identical lines (common in poorly structured HTML)
    let mut result = Vec::new();
    let mut prev = "";
    for line in &lines {
        if *line != prev {
            result.push(*line);
            prev = line;
        }
    }

    result.join("\n")
}

/// Remove all content between <tag...> and </tag> (case-insensitive)
fn remove_blocks(html: &str, tag: &str) -> String {
    let open_pattern = format!("<{}", tag);
    let close_pattern = format!("</{}>", tag);
    let mut result = String::new();
    let lower = html.to_lowercase();
    let mut pos = 0;

    loop {
        let open_start = match lower[pos..].find(&open_pattern) {
            Some(p) => pos + p,
            None => {
                result.push_str(&html[pos..]);
                break;
            }
        };

        // Make sure it's actually a tag (followed by space, >, or end)
        let after_tag = open_start + open_pattern.len();
        if after_tag < lower.len() {
            let next_char = lower.as_bytes()[after_tag];
            if next_char != b' ' && next_char != b'>' && next_char != b'\n' && next_char != b'\t' {
                result.push_str(&html[pos..after_tag]);
                pos = after_tag;
                continue;
            }
        }

        result.push_str(&html[pos..open_start]);

        // Find closing tag
        let close_start = match lower[open_start..].find(&close_pattern) {
            Some(p) => open_start + p + close_pattern.len(),
            None => html.len(), // unclosed tag — skip rest
        };

        pos = close_start;
    }

    result
}

/// Try to extract content from a specific HTML section
fn extract_section(html: &str, hint: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let hint_lower = hint.to_lowercase();

    // Try to find opening tag containing the hint
    let patterns = [
        format!("<{}", hint_lower),           // <main, <article
        format!("class=\"{}", hint_lower),    // class="content"
        format!("id=\"{}", hint_lower),       // id="content"
        format!("role=\"{}", hint_lower),     // role="main"
    ];

    for pattern in &patterns {
        if let Some(start) = lower.find(pattern.as_str()) {
            // Find the > that closes this opening tag
            let tag_end = lower[start..].find('>')? + start + 1;

            // Find the matching closing tag (simplified: find next </tag>)
            // For class/id/role matches, we need to find the tag name
            let tag_name = if pattern.starts_with('<') {
                hint_lower.split_whitespace().next().unwrap_or(&hint_lower)
            } else {
                // Extract tag name from context
                let before = &lower[..start];
                let last_open = before.rfind('<')?;
                let tag_start = last_open + 1;
                let tag_end_pos = lower[tag_start..].find(|c: char| c == ' ' || c == '>')? + tag_start;
                &lower[tag_start..tag_end_pos]
            };

            let close = format!("</{}>", tag_name);
            if let Some(close_pos) = lower[tag_end..].find(&close) {
                let content = &html[tag_end..tag_end + close_pos];
                if content.len() > 100 {
                    return Some(content.to_string());
                }
            }
        }
    }

    None
}

/// Strip all HTML tags from text
fn strip_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut prev_was_space = false;

    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if ch == '>' {
            in_tag = false;
            // Add space after block-level tags
            if !prev_was_space {
                result.push(' ');
                prev_was_space = true;
            }
            continue;
        }
        if !in_tag {
            if ch == '\n' || ch == '\r' {
                if !prev_was_space {
                    result.push('\n');
                    prev_was_space = true;
                }
            } else if ch == ' ' || ch == '\t' {
                if !prev_was_space {
                    result.push(' ');
                    prev_was_space = true;
                }
            } else {
                result.push(ch);
                prev_was_space = false;
            }
        }
    }

    // Decode common HTML entities
    result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&#x27;", "'")
        .replace("&#x2F;", "/")
}
