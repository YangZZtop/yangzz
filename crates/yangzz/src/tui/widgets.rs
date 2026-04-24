use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, Wrap,
};

// Neon palette — GOLD is the signature primary, everything else gets its own
// high-saturation color for maximum differentiation. Tuned to stay readable
// on light terminal backgrounds (darker than pure neon, but still vivid).
//
// Role mapping:
//   GOLD            signature — frame, logo, bullets, version (star of the show)
//   USER_FG         your messages — electric magenta
//   ASSIST_FG       AI replies — electric blue
//   TOOL_FG         tool names — neon violet
//   INFO_FG         hints / descriptive text — bright teal
//   KEY_FG          keyboard shortcuts — neon green
//   ACCENT_FG       "via" / separators — coral
//   ERROR_FG        errors — neon red
//   CODE_FG         inline/fenced code — warm orange
const GOLD: Color = Color::Rgb(255, 184, 0); // ★ bright neon gold (signature)
const GOLD_DIM: Color = Color::Rgb(200, 140, 0); // slightly dimmer for chat border
const USER_FG: Color = Color::Rgb(255, 0, 153); // electric magenta
const ASSIST_FG: Color = Color::Rgb(30, 136, 255); // electric blue
const TOOL_FG: Color = Color::Rgb(138, 43, 226); // neon violet
const INFO_FG: Color = Color::Rgb(0, 191, 165); // bright teal
const KEY_FG: Color = Color::Rgb(0, 200, 80); // neon green — keybind chips
const ACCENT_FG: Color = Color::Rgb(255, 110, 64); // coral orange — separators / "via"
const ERROR_FG: Color = Color::Rgb(255, 23, 68); // neon red
const CODE_FG: Color = Color::Rgb(255, 110, 64); // coral — code
const CODE_BG: Color = Color::Rgb(250, 240, 220); // pale cream — code bg

/// A single entry in the chat history
#[derive(Clone)]
pub enum ChatEntry {
    User(String),
    Assistant(String),
    Tool {
        name: String,
        result: String,
        is_error: bool,
    },
    Info(String),
    Error(String),
    /// Raw logo/banner line — no prefix, bold gold. Reserved for welcome.
    Banner(String),
    /// Fully pre-styled lines — used by welcome hints so each segment can
    /// have its own color. Renders as-is, no prefix applied.
    Custom(Vec<Line<'static>>),
}

impl ChatEntry {
    pub fn to_lines(&self) -> Vec<Line<'static>> {
        match self {
            ChatEntry::User(text) => {
                // Gold chip ❯ on your input, then cyan text so it's clearly
                // "yours" and different from AI replies.
                vec![Line::from(vec![
                    Span::styled(" ❯ ", Style::default().bg(GOLD).fg(Color::Black).bold()),
                    Span::raw("  "),
                    Span::styled(text.clone(), Style::default().fg(USER_FG).bold()),
                ])]
            }
            ChatEntry::Assistant(text) => render_markdown_lines(text),
            ChatEntry::Tool {
                name,
                result,
                is_error,
            } => {
                let icon = if *is_error { "✖" } else { "●" };
                let icon_color = if *is_error {
                    ERROR_FG
                } else {
                    Color::Rgb(0, 160, 80)
                };
                let mut lines = vec![Line::from(vec![
                    Span::styled(format!("  {icon} "), Style::default().fg(icon_color)),
                    Span::styled(name.clone(), Style::default().fg(TOOL_FG).bold()),
                    Span::styled(
                        if *is_error { " ✖" } else { " ✓" },
                        Style::default().fg(icon_color),
                    ),
                ])];
                if let Some(first) = result.lines().next() {
                    let preview: String = first.chars().take(80).collect();
                    if !preview.is_empty() {
                        lines.push(Line::from(vec![
                            Span::styled("  ⎿ ", Style::default().fg(INFO_FG)),
                            Span::styled(preview, Style::default().fg(INFO_FG)),
                        ]));
                    }
                }
                lines
            }
            ChatEntry::Info(text) => {
                vec![Line::from(vec![
                    // Bright gold bullet so Info lines are clearly grouped
                    Span::styled("  • ", Style::default().fg(GOLD).bold()),
                    Span::styled(text.clone(), Style::default().fg(INFO_FG)),
                ])]
            }
            ChatEntry::Error(text) => {
                vec![Line::from(vec![
                    Span::styled("  ✖ ", Style::default().fg(ERROR_FG).bold()),
                    Span::styled(text.clone(), Style::default().fg(ERROR_FG).bold()),
                ])]
            }
            ChatEntry::Banner(text) => {
                // No prefix, bold gold — for the yangzz logo block.
                vec![Line::from(vec![Span::styled(
                    text.clone(),
                    Style::default().fg(GOLD).bold(),
                )])]
            }
            ChatEntry::Custom(lines) => lines.clone(),
        }
    }
}

/// Build a one-line welcome entry with **differentiated neon colors**.
/// Each segment gets its own role color so no two look alike.
pub fn welcome_line_version_model_provider(
    version: &str,
    model: &str,
    provider: &str,
) -> Vec<Line<'static>> {
    vec![Line::from(vec![
        Span::styled("  • ", Style::default().fg(GOLD).bold()),
        // Version → signature gold
        Span::styled(
            format!("yangzz v{version}"),
            Style::default().fg(GOLD).bold(),
        ),
        Span::styled("   ", Style::default()),
        // Model → electric blue
        Span::styled(model.to_string(), Style::default().fg(ASSIST_FG).bold()),
        // "via" → coral
        Span::styled("  via  ", Style::default().fg(ACCENT_FG).bold()),
        // Provider → neon violet
        Span::styled(provider.to_string(), Style::default().fg(TOOL_FG).bold()),
    ])]
}

/// Welcome hint line — keys in neon green, slash commands in violet,
/// descriptions in teal, separators in coral. No two adjacent elements
/// share a color.
pub fn welcome_line_hints() -> Vec<Line<'static>> {
    let key_style = Style::default().fg(KEY_FG).bold();
    let desc_style = Style::default().fg(INFO_FG);
    let cmd_style = Style::default().fg(TOOL_FG).bold();
    let sep = Span::styled("  ·  ", Style::default().fg(ACCENT_FG).bold());
    vec![Line::from(vec![
        Span::styled("  • ", Style::default().fg(GOLD).bold()),
        Span::styled("Ctrl+D", key_style),
        Span::styled(" 退出", desc_style),
        sep.clone(),
        Span::styled("/help", cmd_style),
        Span::styled(" 命令", desc_style),
        sep.clone(),
        Span::styled("/model", cmd_style),
        Span::styled(" 切换", desc_style),
        sep.clone(),
        Span::styled("/clear", cmd_style),
        Span::styled(" 清空", desc_style),
        sep.clone(),
        Span::styled("滚轮翻页", key_style),
        sep,
        Span::styled("Shift+拖拽选中", key_style),
    ])]
}

/// Parse assistant text as light markdown → styled ratatui Lines.
/// Supports: headings (# ## ###), bold (**text**), inline code (`text`),
/// fenced code blocks (```), and bullet lists (- item). Not a full markdown
/// parser — covers the 80% of what LLMs emit in chat.
fn render_markdown_lines(text: &str) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut in_code_block = false;

    for (i, raw_line) in text.lines().enumerate() {
        let prefix = if i == 0 { "⎿ " } else { "  " };
        let prefix_span = Span::styled(prefix.to_string(), Style::default().fg(GOLD));

        // Fenced code block toggle
        if raw_line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            out.push(Line::from(vec![
                prefix_span,
                Span::styled(raw_line.to_string(), Style::default().fg(GOLD_DIM)),
            ]));
            continue;
        }

        if in_code_block {
            out.push(Line::from(vec![
                prefix_span,
                Span::styled(
                    raw_line.to_string(),
                    Style::default().fg(CODE_FG).bg(CODE_BG),
                ),
            ]));
            continue;
        }

        // Heading — match leading #
        let trimmed = raw_line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("### ") {
            out.push(Line::from(vec![
                prefix_span,
                Span::styled(format!("▸ {rest}"), Style::default().fg(GOLD).bold()),
            ]));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            out.push(Line::from(vec![
                prefix_span,
                Span::styled(format!("▾ {rest}"), Style::default().fg(GOLD).bold()),
            ]));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            out.push(Line::from(vec![
                prefix_span,
                Span::styled(format!("■ {rest}"), Style::default().fg(GOLD).bold()),
            ]));
            continue;
        }

        // Bullet — keep it, restyle marker
        let (bullet_prefix, content) = if let Some(rest) = trimmed.strip_prefix("- ") {
            ("• ", rest)
        } else if let Some(rest) = trimmed.strip_prefix("* ") {
            ("• ", rest)
        } else {
            ("", raw_line)
        };

        // Inline bold / code parsing — wrap everything in ASSIST_FG so regular
        // text reads as warm off-white (distinct from user's cyan).
        let mut spans: Vec<Span<'static>> = vec![prefix_span];
        if !bullet_prefix.is_empty() {
            spans.push(Span::styled(
                bullet_prefix.to_string(),
                Style::default().fg(GOLD),
            ));
        }
        // Wrap base spans with assistant fg; parse_inline emits already-styled
        // spans for bold/code which override base style naturally.
        for sp in parse_inline(content) {
            // If the span is already styled explicitly (code/bold), leave it.
            // Otherwise, apply ASSIST_FG so it has a readable color.
            if sp.style == Style::default() {
                spans.push(Span::styled(sp.content, Style::default().fg(ASSIST_FG)));
            } else {
                spans.push(sp);
            }
        }

        out.push(Line::from(spans));
    }

    out
}

/// Parse a single line for inline markdown: **bold** and `inline code`.
fn parse_inline(s: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut buf = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        // Inline code: `...`
        if c == '`' {
            if !buf.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut buf)));
            }
            let mut code = String::new();
            for nc in chars.by_ref() {
                if nc == '`' {
                    break;
                }
                code.push(nc);
            }
            spans.push(Span::styled(code, Style::default().fg(CODE_FG).bg(CODE_BG)));
            continue;
        }
        // Bold: **...**
        if c == '*' && chars.peek() == Some(&'*') {
            chars.next(); // consume second *
            if !buf.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut buf)));
            }
            let mut bold = String::new();
            while let Some(nc) = chars.next() {
                if nc == '*' && chars.peek() == Some(&'*') {
                    chars.next();
                    break;
                }
                bold.push(nc);
            }
            spans.push(Span::styled(bold, Style::default().bold()));
            continue;
        }
        buf.push(c);
    }

    if !buf.is_empty() {
        spans.push(Span::raw(buf));
    }
    spans
}

/// Render the chat history area
pub fn render_chat(
    frame: &mut Frame,
    area: Rect,
    entries: &[ChatEntry],
    scroll_offset: u16,
    is_thinking: bool,
) {
    let mut all_lines: Vec<Line<'static>> = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        all_lines.extend(entry.to_lines());
        // No blank line between two consecutive Banner entries — the logo
        // should render as one continuous block. Everything else gets a
        // blank trailing line for visual separation.
        let next_is_banner = entries
            .get(i + 1)
            .map(|e| matches!(e, ChatEntry::Banner(_)))
            .unwrap_or(false);
        let this_is_banner = matches!(entry, ChatEntry::Banner(_));
        if !(this_is_banner && next_is_banner) {
            all_lines.push(Line::from(""));
        }
    }
    if is_thinking {
        all_lines.push(Line::from(vec![
            Span::styled("  ∴ ", Style::default().fg(Color::Rgb(218, 165, 32))),
            Span::styled("Thinking...", Style::default().fg(Color::DarkGray).italic()),
        ]));
    }

    // Build the Paragraph with wrap FIRST so we can query line_count(),
    // which accounts for line-wrapping (a 100-char error on a 50-col
    // terminal counts as 2 visual lines, not 1).
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(GOLD_DIM))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(Span::styled(" yangzz ", Style::default().fg(GOLD).bold()));

    let para = Paragraph::new(all_lines)
        .block(block)
        .wrap(Wrap { trim: false });

    // scroll_offset meaning: lines scrolled UP from the bottom.
    //   0 → stuck to bottom (newest). Increments on PageUp.
    let content_width = area.width.saturating_sub(2); // minus border
    let content_height = para.line_count(content_width) as u16;
    let visible_height = area.height.saturating_sub(2); // minus border
    let max_top_offset = content_height.saturating_sub(visible_height);
    let clamped_up = scroll_offset.min(max_top_offset);
    let effective_scroll = max_top_offset.saturating_sub(clamped_up);

    let para = para.scroll((effective_scroll, 0));

    frame.render_widget(para, area);

    // Right-edge scrollbar — only show when content actually overflows.
    // Position reflects effective_scroll (lines from top of content).
    if max_top_offset > 0 {
        let total = content_height as usize;
        let pos = effective_scroll as usize;
        let mut sb_state = ScrollbarState::new(total).position(pos);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .thumb_symbol("█")
            .track_symbol(Some("│"))
            .style(Style::default().fg(GOLD_DIM))
            .thumb_style(Style::default().fg(GOLD).bold());
        frame.render_stateful_widget(scrollbar, area, &mut sb_state);
    }
}

/// Render the input line
pub fn render_input(frame: &mut Frame, area: Rect, input: &str, cursor_pos: usize, label: &str) {
    let mut spans = vec![
        Span::styled(
            " ❯ ",
            Style::default()
                .bg(Color::Rgb(218, 165, 32))
                .fg(Color::Black)
                .bold(),
        ),
        Span::raw(" "),
    ];
    if !label.is_empty() {
        spans.push(Span::styled(
            format!("{label} "),
            Style::default().fg(Color::DarkGray),
        ));
    }
    // Your typed input uses the same hot-magenta as your sent messages so
    // there's visual continuity (and it stays visible on light terminals).
    spans.push(Span::styled(
        input.to_string(),
        Style::default().fg(USER_FG).bold(),
    ));

    let para = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(GOLD))
            .border_type(ratatui::widgets::BorderType::Rounded),
    );
    frame.render_widget(para, area);

    // Place cursor — cursor_pos is a BYTE index; convert to display width
    // accounting for CJK wide chars (2 cols each).
    let text_before_cursor = &input[..cursor_pos.min(input.len())];
    let display_cols: u16 = text_before_cursor
        .chars()
        .map(|c| if is_wide_char(c) { 2u16 } else { 1u16 })
        .sum();
    let label_offset = if label.is_empty() {
        0
    } else {
        label.len() as u16 + 1
    };
    frame.set_cursor_position((area.x + 5 + label_offset + display_cols, area.y + 1));
}

/// True if the character renders with width 2 in a monospace terminal (CJK,
/// emoji, etc). Used for cursor position computation.
fn is_wide_char(c: char) -> bool {
    // Covers CJK Unified, Hiragana, Katakana, fullwidth, CJK Extension A/B,
    // Hangul, emoji Misc Symbols. Not exhaustive but 95%+ accurate.
    matches!(
        c as u32,
        0x1100..=0x115F |
        0x2E80..=0x303E |
        0x3041..=0x33FF |
        0x3400..=0x4DBF |
        0x4E00..=0x9FFF |
        0xA000..=0xA4CF |
        0xAC00..=0xD7A3 |
        0xF900..=0xFAFF |
        0xFE30..=0xFE4F |
        0xFF00..=0xFF60 |
        0xFFE0..=0xFFE6 |
        0x1F300..=0x1FAFF |
        0x20000..=0x2FFFD |
        0x30000..=0x3FFFD
    )
}

/// Render a floating suggestion popup above the input area. Shows up to 6
/// matches with the currently selected one highlighted in gold. Mimics the
/// Codex CLI feel without needing to rewrite the whole input loop.
pub fn render_suggestions(
    frame: &mut Frame,
    input_area: Rect,
    suggestions: &[(String, String)],
    selected: usize,
) {
    if suggestions.is_empty() {
        return;
    }

    // Max 6 rows visible; adjust if fewer suggestions
    let max_visible = suggestions.len().min(6);
    // 2 for top+bottom border
    let height = (max_visible + 2).min(10) as u16;
    // Anchor above the input area, same width-ish (trim to a sane max)
    let popup_width = input_area.width.min(70).max(30);
    let popup_x = input_area.x;
    let popup_y = input_area.y.saturating_sub(height);

    let popup_area = Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height,
    };

    // Window: if selected is beyond max_visible, scroll
    let start = if selected >= max_visible {
        selected + 1 - max_visible
    } else {
        0
    };
    let end = (start + max_visible).min(suggestions.len());

    let items: Vec<ListItem> = suggestions[start..end]
        .iter()
        .enumerate()
        .map(|(i, (name, desc))| {
            let abs = start + i;
            let is_sel = abs == selected;
            let name_style = if is_sel {
                Style::default().fg(Color::Black).bg(GOLD).bold()
            } else {
                Style::default().fg(GOLD).bold()
            };
            let desc_style = if is_sel {
                Style::default().fg(Color::Black).bg(GOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let pad_len = 18usize.saturating_sub(name.len());
            let pad: String = std::iter::repeat_n(' ', pad_len).collect();
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {name}"), name_style),
                Span::styled(pad, desc_style),
                Span::styled(format!("  {desc} "), desc_style),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(GOLD))
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(Span::styled(
                " ↑↓ 选择  ·  Tab 采纳  ·  Enter 发送  ·  Esc 关闭 ",
                Style::default().fg(GOLD_DIM),
            )),
    );

    // Clear underneath so background shows through cleanly
    frame.render_widget(Clear, popup_area);
    frame.render_widget(list, popup_area);
}

/// Render a centered modal dialog asking the user to approve a tool invocation.
/// Uses gold border + background dimmed by a full-screen Clear so the modal
/// clearly interrupts normal flow.
pub fn render_permission_modal(
    frame: &mut Frame,
    full_area: Rect,
    ask: &yangzz_core::permission::PermissionAsk,
) {
    // Center a 60%-wide 11-high modal
    let w = (full_area.width.saturating_sub(10)).min(72).max(40);
    let h: u16 = 11;
    let x = full_area.x + (full_area.width.saturating_sub(w)) / 2;
    let y = full_area.y + (full_area.height.saturating_sub(h)) / 2;
    let area = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    // Summarize the input JSON — keep short
    let input_preview = summarize_input(&ask.input);

    let lines: Vec<Line<'static>> = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("工具请求授权：", Style::default().fg(Color::White).bold()),
            Span::styled(
                format!("{}", ask.tool_name),
                Style::default().fg(GOLD).bold(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(input_preview, Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("[y]", Style::default().fg(Color::Green).bold()),
            Span::raw(" 允许   "),
            Span::styled("[n]", Style::default().fg(Color::Red).bold()),
            Span::raw(" 拒绝   "),
            Span::styled("[a]", Style::default().fg(GOLD).bold()),
            Span::raw(" 始终允许"),
        ]),
    ];

    let title = if ask.is_destructive {
        " ⚠ 危险操作确认 "
    } else {
        " 🔐 权限确认 "
    };

    let modal = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(GOLD).bold())
            .border_type(ratatui::widgets::BorderType::Double)
            .title(Span::styled(title, Style::default().fg(GOLD).bold())),
    );

    frame.render_widget(Clear, area);
    frame.render_widget(modal, area);
}

fn summarize_input(v: &serde_json::Value) -> String {
    // Friendly 1-line summary of key fields LLMs put in tool inputs.
    if let Some(cmd) = v.get("command").and_then(|x| x.as_str()) {
        let trunc: String = cmd.chars().take(60).collect();
        return format!("$ {trunc}{}", if cmd.len() > 60 { "…" } else { "" });
    }
    if let Some(p) = v.get("path").and_then(|x| x.as_str()) {
        return format!("path = {p}");
    }
    if let Some(p) = v.get("file_path").and_then(|x| x.as_str()) {
        return format!("file = {p}");
    }
    if let Some(u) = v.get("url").and_then(|x| x.as_str()) {
        return format!("url = {u}");
    }
    // Fallback: truncated JSON
    let s = v.to_string();
    let short: String = s.chars().take(80).collect();
    if s.len() > short.len() {
        format!("{short}…")
    } else {
        short
    }
}

/// Render the status bar
pub fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    model: &str,
    _provider: &str,
    total_tokens: u32,
    cost_usd: f64,
) {
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "~".into());

    let now = chrono::Local::now().format("%H:%M").to_string();

    let sep = Span::styled(" │ ", Style::default().fg(Color::DarkGray));

    let mut spans = vec![
        Span::styled(format!(" {cwd}"), Style::default().fg(Color::DarkGray)),
        sep.clone(),
        Span::styled(
            model.to_string(),
            Style::default().fg(Color::Rgb(218, 165, 32)),
        ),
    ];

    if total_tokens > 0 {
        spans.push(sep.clone());
        spans.push(Span::styled(
            format!("{total_tokens} token"),
            Style::default().fg(Color::DarkGray),
        ));
    }
    if cost_usd > 0.0001 {
        spans.push(sep.clone());
        spans.push(Span::styled(
            format!("${cost_usd:.4}"),
            Style::default().fg(Color::Green),
        ));
    }
    spans.push(sep);
    spans.push(Span::styled(now, Style::default().fg(Color::DarkGray)));
    spans.push(Span::raw(" "));

    let bar = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(GOLD_DIM)),
    );
    frame.render_widget(bar, area);
}
