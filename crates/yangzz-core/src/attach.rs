//! Parse `@path/to/file` attachments out of a user input line.
//!
//! Shared by REPL and TUI so both get the same feel:
//!   "解释这张图 @/tmp/shot.png 和这个文件 @./src/main.rs"
//!   "帮我看看 @Cargo.toml 有什么问题"
//!
//! Extraction rules:
//! - `@` followed by a path-looking token (starts with `/`, `./`, `~/`,
//!   or contains a `.` extension, or is a known filename like Makefile).
//! - Token is read until the next whitespace.
//! - Image files (png/jpg/jpeg/gif/webp): loaded as base64 image blocks.
//! - Text files: content read and inlined as text (with filename header).
//! - Max image size: 5 MB. Max text file size: 512 KB.

use base64::Engine;

use crate::message::{ContentBlock, Message, Role};

const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_TEXT_BYTES: u64 = 512 * 1024;

/// Result of turning a user line into a Message.
pub struct AttachParseResult {
    pub message: Message,
    /// Human-readable summary of what was attached, for the UI.
    pub attachments_summary: String,
    /// Any issues encountered (file missing, too big, etc.). UI should show.
    pub warnings: Vec<String>,
}

/// Parse one line of user input. Returns the built Message + info for the UI.
pub fn parse_input(line: &str) -> AttachParseResult {
    let mut warnings = Vec::new();
    let mut images: Vec<ContentBlock> = Vec::new();
    let mut file_texts: Vec<(String, String)> = Vec::new(); // (filename, content)
    let mut text_parts: Vec<String> = Vec::new();

    // Walk tokens separated by whitespace.
    let mut current = String::new();
    let mut tokens: Vec<String> = Vec::new();
    for ch in line.chars() {
        if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            tokens.push(ch.to_string());
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    for tok in &tokens {
        if let Some(path_str) = tok.strip_prefix('@') {
            if path_str.is_empty() {
                text_parts.push(tok.clone());
                continue;
            }

            if looks_like_image_path(path_str) {
                // Image attachment
                match load_image(path_str) {
                    Ok(block) => {
                        images.push(block);
                        text_parts.push(format!("[image:{}]", file_name_of(path_str)));
                    }
                    Err(e) => {
                        warnings.push(format!("{tok}: {e}"));
                        text_parts.push(tok.clone());
                    }
                }
            } else if looks_like_file_path(path_str) {
                // Text file attachment
                match load_text_file(path_str) {
                    Ok((filename, content)) => {
                        file_texts.push((filename.clone(), content));
                        text_parts.push(format!("[file:{}]", filename));
                    }
                    Err(e) => {
                        warnings.push(format!("{tok}: {e}"));
                        text_parts.push(tok.clone());
                    }
                }
            } else {
                // Not a recognizable path — keep as-is
                text_parts.push(tok.clone());
            }
            continue;
        }
        text_parts.push(tok.clone());
    }

    // Build the message text: user text + inlined file contents
    let user_text = text_parts.join("");
    let mut full_text = String::new();
    if !user_text.trim().is_empty() {
        full_text.push_str(&user_text);
    }

    // Append file contents after the user's text
    if !file_texts.is_empty() {
        if !full_text.is_empty() {
            full_text.push_str("\n\n");
        }
        for (filename, content) in &file_texts {
            full_text.push_str(&format!(
                "--- File: {filename} ---\n{content}\n--- End: {filename} ---\n\n"
            ));
        }
    }

    let mut content: Vec<ContentBlock> = Vec::new();
    if !full_text.trim().is_empty() {
        content.push(ContentBlock::text(full_text));
    }
    content.extend(images.iter().cloned());
    if content.is_empty() {
        content.push(ContentBlock::text(line.to_string()));
    }

    // Build summary
    let total_attachments = images.len() + file_texts.len();
    let summary = if total_attachments == 0 {
        String::new()
    } else {
        let mut parts = Vec::new();
        if !images.is_empty() {
            parts.push(format!(
                "{} image{}",
                images.len(),
                if images.len() > 1 { "s" } else { "" }
            ));
        }
        if !file_texts.is_empty() {
            parts.push(format!(
                "{} file{}",
                file_texts.len(),
                if file_texts.len() > 1 { "s" } else { "" }
            ));
        }
        format!("+ {} attached", parts.join(" + "))
    };

    AttachParseResult {
        message: Message {
            role: Role::User,
            content,
        },
        attachments_summary: summary,
        warnings,
    }
}

/// Check if a path looks like an image file
fn looks_like_image_path(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
}

/// Check if a token looks like a file path (not just a random word after @)
fn looks_like_file_path(s: &str) -> bool {
    // Starts with path-like prefix
    if s.starts_with('/')
        || s.starts_with("./")
        || s.starts_with("../")
        || s.starts_with("~/")
    {
        return true;
    }

    // Contains a path separator (subdirectory reference)
    if s.contains('/') {
        return true;
    }

    // Has a file extension (contains a dot with something after it)
    if let Some(dot_pos) = s.rfind('.') {
        let ext = &s[dot_pos + 1..];
        // Must have a reasonable extension (1-10 chars, alphanumeric)
        if !ext.is_empty() && ext.len() <= 10 && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
            return true;
        }
    }

    // Known extensionless filenames
    let known_files = [
        "Makefile",
        "Dockerfile",
        "Cargo.lock",
        "Gemfile",
        "Rakefile",
        "Procfile",
        "Vagrantfile",
        "Jenkinsfile",
        "MEMORY",
        "README",
        "LICENSE",
        "CHANGELOG",
    ];
    if known_files.iter().any(|f| s == *f) {
        return true;
    }

    false
}

fn file_name_of(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| p.to_string())
}

fn media_type_for(path: &str) -> Option<&'static str> {
    let lower = path.to_lowercase();
    if lower.ends_with(".png") {
        Some("image/png")
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        Some("image/jpeg")
    } else if lower.ends_with(".gif") {
        Some("image/gif")
    } else if lower.ends_with(".webp") {
        Some("image/webp")
    } else {
        None
    }
}

fn expand_tilde(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path)
}

fn load_image(path: &str) -> Result<ContentBlock, String> {
    let mt = media_type_for(path).ok_or_else(|| "不支持的图片格式".to_string())?;
    let abs = expand_tilde(path);
    if !abs.exists() {
        return Err(format!("文件不存在: {}", abs.display()));
    }
    let size = abs
        .metadata()
        .map_err(|e| format!("无法读取元数据: {e}"))?
        .len();
    if size > MAX_IMAGE_BYTES {
        return Err(format!(
            "图片太大 ({:.1} MB)，上限 5 MB",
            size as f64 / 1_048_576.0
        ));
    }
    let bytes = std::fs::read(&abs).map_err(|e| format!("读文件失败: {e}"))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(ContentBlock::image(mt, b64))
}

/// Load a text file and return (filename, content)
fn load_text_file(path: &str) -> Result<(String, String), String> {
    let abs = expand_tilde(path);

    // Also try relative to cwd
    let resolved = if abs.exists() {
        abs
    } else if let Ok(cwd) = std::env::current_dir() {
        let relative = cwd.join(path);
        if relative.exists() {
            relative
        } else {
            return Err(format!("文件不存在: {} (也不在 {})", abs.display(), relative.display()));
        }
    } else {
        return Err(format!("文件不存在: {}", abs.display()));
    };

    let size = resolved
        .metadata()
        .map_err(|e| format!("无法读取元数据: {e}"))?
        .len();

    if size > MAX_TEXT_BYTES {
        return Err(format!(
            "文件太大 ({:.1} KB)，文本附件上限 512 KB",
            size as f64 / 1024.0
        ));
    }

    // Check if it's likely a binary file
    let bytes = std::fs::read(&resolved).map_err(|e| format!("读文件失败: {e}"))?;

    // Quick binary check: if first 8KB has null bytes, it's probably binary
    let check_len = bytes.len().min(8192);
    let has_null = bytes[..check_len].contains(&0);
    if has_null {
        return Err(format!(
            "看起来是二进制文件，不支持作为文本附件: {}",
            resolved.display()
        ));
    }

    let content = String::from_utf8(bytes).map_err(|_| {
        format!("文件不是有效 UTF-8 文本: {}", resolved.display())
    })?;

    let filename = file_name_of(path);

    // If file is very large, truncate with notice
    if content.len() > 100_000 {
        let truncated: String = content.chars().take(100_000).collect();
        Ok((
            filename,
            format!("{truncated}\n\n... (文件过长，已截断至前 100K 字符)"),
        ))
    } else {
        Ok((filename, content))
    }
}
