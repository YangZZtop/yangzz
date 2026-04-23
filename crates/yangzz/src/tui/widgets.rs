use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// A single entry in the chat history
#[derive(Clone)]
pub enum ChatEntry {
    User(String),
    Assistant(String),
    Tool { name: String, result: String, is_error: bool },
    Info(String),
    Error(String),
}

impl ChatEntry {
    pub fn to_lines(&self) -> Vec<Line<'static>> {
        match self {
            ChatEntry::User(text) => {
                vec![Line::from(vec![
                    Span::styled(" ❯ ", Style::default().fg(Color::Rgb(218, 165, 32)).bold()),
                    Span::styled(text.clone(), Style::default().fg(Color::White).bold()),
                ])]
            }
            ChatEntry::Assistant(text) => {
                text.lines()
                    .enumerate()
                    .map(|(i, line)| {
                        let prefix = if i == 0 { "⎿ " } else { "  " };
                        Line::from(vec![
                            Span::styled(prefix.to_string(), Style::default().fg(Color::Rgb(218, 165, 32))),
                            Span::raw(line.to_string()),
                        ])
                    })
                    .collect()
            }
            ChatEntry::Tool { name, result, is_error } => {
                let icon = if *is_error { "✖" } else { "●" };
                let color = if *is_error { Color::Red } else { Color::Green };
                let mut lines = vec![Line::from(vec![
                    Span::styled(format!("  {icon} "), Style::default().fg(color)),
                    Span::styled(name.clone(), Style::default().fg(Color::White).bold()),
                    Span::styled(if *is_error { " ✖" } else { " ✓" }, Style::default().fg(color)),
                ])];
                // Show first line of result as preview
                if let Some(first) = result.lines().next() {
                    let preview: String = first.chars().take(80).collect();
                    if !preview.is_empty() {
                        lines.push(Line::from(vec![
                            Span::styled("  ⎿ ", Style::default().fg(Color::DarkGray)),
                            Span::styled(preview, Style::default().fg(Color::DarkGray)),
                        ]));
                    }
                }
                lines
            }
            ChatEntry::Info(text) => {
                vec![Line::from(vec![
                    Span::styled("  • ", Style::default().fg(Color::DarkGray)),
                    Span::styled(text.clone(), Style::default().fg(Color::Rgb(218, 165, 32))),
                ])]
            }
            ChatEntry::Error(text) => {
                vec![Line::from(vec![
                    Span::styled("  ✖ ", Style::default().fg(Color::Red)),
                    Span::styled(text.clone(), Style::default().fg(Color::Red)),
                ])]
            }
        }
    }
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
    for entry in entries {
        all_lines.extend(entry.to_lines());
        all_lines.push(Line::from(""));
    }
    if is_thinking {
        all_lines.push(Line::from(vec![
            Span::styled("  ∴ ", Style::default().fg(Color::Rgb(218, 165, 32))),
            Span::styled("Thinking...", Style::default().fg(Color::DarkGray).italic()),
        ]));
    }

    // Auto-scroll: if at bottom, show latest content
    let content_height = all_lines.len() as u16;
    let visible_height = area.height.saturating_sub(2); // minus border
    let effective_scroll = if scroll_offset == 0 {
        content_height.saturating_sub(visible_height)
    } else {
        scroll_offset
    };

    let para = Paragraph::new(all_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    " yangzz ",
                    Style::default().fg(Color::Rgb(218, 165, 32)).bold(),
                )),
        )
        .wrap(Wrap { trim: false })
        .scroll((effective_scroll, 0));

    frame.render_widget(para, area);
}

/// Render the input line
pub fn render_input(frame: &mut Frame, area: Rect, input: &str, cursor_pos: usize, label: &str) {
    let mut spans = vec![
        Span::styled(" ❯ ", Style::default().bg(Color::Rgb(218, 165, 32)).fg(Color::Black).bold()),
        Span::raw(" "),
    ];
    if !label.is_empty() {
        spans.push(Span::styled(format!("{label} "), Style::default().fg(Color::DarkGray)));
    }
    spans.push(Span::styled(input.to_string(), Style::default().fg(Color::White)));

    let para = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(para, area);

    // Place cursor (offset: border + " ❯ " + " " = 5, plus optional label)
    let label_offset = if label.is_empty() { 0 } else { label.len() as u16 + 1 };
    frame.set_cursor_position((area.x + 5 + label_offset + cursor_pos as u16, area.y + 1));
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
        Span::styled(model.to_string(), Style::default().fg(Color::Rgb(218, 165, 32))),
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
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(bar, area);
}
