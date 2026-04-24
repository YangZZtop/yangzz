//! Parse `@path/to/file.png` image attachments out of a user input line.
//!
//! Shared by REPL and TUI so both get the same feel:
//!   "解释这张图 @/tmp/shot.png 和这张 @./design.jpg"
//!
//! Extraction rules (conservative on purpose):
//! - Only when `@` is followed by a path-looking token (starts with `/` or
//!   `./` or `~/`, or just has letters + a supported image extension).
//! - Token is read until the next whitespace.
//! - Supported extensions: png / jpg / jpeg / gif / webp.
//! - Max file size: 5 MB (Anthropic's cap; OpenAI allows more but 5 MB is safe).

use base64::Engine;

use crate::message::{ContentBlock, Message, Role};

const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;

/// Result of turning a user line into a Message. Text-only lines become a
/// simple text Message; lines with `@image` refs become a multi-part Message.
pub struct AttachParseResult {
    pub message: Message,
    /// Human-readable summary of what was attached, for the UI.
    /// E.g. `"+ 2 images attached"` or empty string if nothing.
    pub attachments_summary: String,
    /// Any issues encountered (file missing, too big, etc.). UI should show.
    pub warnings: Vec<String>,
}

/// Parse one line of user input. Returns the built Message + info for the UI.
pub fn parse_input(line: &str) -> AttachParseResult {
    let mut warnings = Vec::new();
    let mut images: Vec<ContentBlock> = Vec::new();
    let mut text_parts: Vec<String> = Vec::new();

    // Walk tokens separated by whitespace. Preserve original whitespace so
    // text reconstruction is faithful.
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
            if looks_like_image_path(path_str) {
                match load_image(path_str) {
                    Ok(block) => {
                        images.push(block);
                        // Don't keep the @path in the text — replace with a placeholder
                        text_parts.push(format!("[image:{}]", file_name_of(path_str)));
                    }
                    Err(e) => {
                        warnings.push(format!("{tok}: {e}"));
                        text_parts.push(tok.clone()); // keep original so model sees user typed it
                    }
                }
                continue;
            }
        }
        text_parts.push(tok.clone());
    }

    let text = text_parts.join("");
    let mut content: Vec<ContentBlock> = Vec::new();
    if !text.trim().is_empty() {
        content.push(ContentBlock::text(text));
    }
    content.extend(images.iter().cloned());
    if content.is_empty() {
        // Edge case — all input was just an @path that failed to load.
        content.push(ContentBlock::text(line.to_string()));
    }

    let summary = if images.is_empty() {
        String::new()
    } else if images.len() == 1 {
        "+ 1 image attached".to_string()
    } else {
        format!("+ {} images attached", images.len())
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

fn looks_like_image_path(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
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
