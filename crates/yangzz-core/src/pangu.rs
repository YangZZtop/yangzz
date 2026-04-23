/// Pangu: insert spaces between CJK and half-width characters.
/// e.g. "你好world" → "你好 world", "hello世界" → "hello 世界"
pub fn spacing(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= 1 {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len() + text.len() / 4);
    result.push(chars[0]);

    for i in 1..chars.len() {
        let prev = chars[i - 1];
        let curr = chars[i];

        // CJK followed by ASCII alphanumeric, or vice versa
        if (is_cjk(prev) && is_half_width_content(curr))
            || (is_half_width_content(prev) && is_cjk(curr))
        {
            result.push(' ');
        }

        result.push(curr);
    }

    result
}

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{2E80}'..='\u{2FFF}' |   // CJK Radicals
        '\u{3040}'..='\u{309F}' |   // Hiragana
        '\u{30A0}'..='\u{30FF}' |   // Katakana
        '\u{3400}'..='\u{4DBF}' |   // CJK Unified Extension A
        '\u{4E00}'..='\u{9FFF}' |   // CJK Unified Ideographs
        '\u{F900}'..='\u{FAFF}' |   // CJK Compatibility Ideographs
        '\u{FE30}'..='\u{FE4F}' |   // CJK Compatibility Forms
        '\u{20000}'..='\u{2A6DF}' | // CJK Extension B
        '\u{2A700}'..='\u{2CEAF}' | // CJK Extension C/D/E/F
        '\u{2CEB0}'..='\u{2EBEF}' | // CJK Extension G
        '\u{30000}'..='\u{3134F}'   // CJK Extension H
    )
}

fn is_half_width_content(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cjk_ascii() {
        assert_eq!(spacing("你好world"), "你好 world");
        assert_eq!(spacing("hello世界"), "hello 世界");
        assert_eq!(spacing("你好"), "你好");
        assert_eq!(spacing("hello"), "hello");
        assert_eq!(spacing("前端Front后端Backend"), "前端 Front 后端 Backend");
        assert_eq!(spacing("使用Rust编写"), "使用 Rust 编写");
    }

    #[test]
    fn test_already_spaced() {
        assert_eq!(spacing("你好 world"), "你好 world");
    }

    #[test]
    fn test_empty() {
        assert_eq!(spacing(""), "");
        assert_eq!(spacing("x"), "x");
    }
}
