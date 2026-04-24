use tokio::sync::mpsc;
use yangzz_core::message::Message;
use yangzz_core::render::Renderer;

/// Messages from the Renderer to the TUI event loop
#[derive(Debug, Clone)]
pub enum UiEvent {
    /// Append/update text delta (streaming)
    TextDelta(String),
    /// Flush accumulated text as a complete assistant message
    AssistantComplete(String),
    /// Tool started
    ToolStart { name: String },
    /// Tool result
    ToolResult {
        name: String,
        result: String,
        is_error: bool,
    },
    /// Error message
    Error(String),
    /// Info message
    Info(String),
    /// Thinking started
    ThinkingStart,
    /// Thinking stopped
    ThinkingStop,
    /// Response complete with usage
    ResponseComplete,
    /// Sync usage data back to state
    UsageUpdate {
        input_tokens: u32,
        output_tokens: u32,
    },
    /// Sync messages back to state after agentic loop
    MessagesSync(Vec<Message>),
}

/// A Renderer implementation that sends events via channel to the TUI
pub struct TuiRenderer {
    tx: mpsc::UnboundedSender<UiEvent>,
    streaming_text: String,
    first_token: bool,
}

impl TuiRenderer {
    pub fn new(tx: mpsc::UnboundedSender<UiEvent>) -> Self {
        Self {
            tx,
            streaming_text: String::new(),
            first_token: true,
        }
    }

    fn send(&self, event: UiEvent) {
        let _ = self.tx.send(event);
    }
}

impl Renderer for TuiRenderer {
    fn render_text_delta(&mut self, text: &str) {
        if self.first_token {
            self.first_token = false;
            self.send(UiEvent::ThinkingStop);
        }
        self.streaming_text.push_str(text);
        self.send(UiEvent::TextDelta(self.streaming_text.clone()));
    }

    fn render_tool_start(&mut self, name: &str, _id: &str) {
        // Flush any pending text
        if !self.streaming_text.is_empty() {
            let text = std::mem::take(&mut self.streaming_text);
            self.send(UiEvent::AssistantComplete(text));
        }
        self.send(UiEvent::ToolStart {
            name: name.to_string(),
        });
    }

    fn render_tool_result(&mut self, name: &str, result: &str, is_error: bool) {
        self.send(UiEvent::ToolResult {
            name: name.to_string(),
            result: result.to_string(),
            is_error,
        });
    }

    fn render_error(&mut self, message: &str) {
        self.send(UiEvent::ThinkingStop);
        self.send(UiEvent::Error(message.to_string()));
    }

    fn render_info(&mut self, message: &str) {
        self.send(UiEvent::Info(message.to_string()));
    }

    fn render_complete(&mut self) {
        if !self.streaming_text.is_empty() {
            let text = std::mem::take(&mut self.streaming_text);
            self.send(UiEvent::AssistantComplete(text));
        }
        self.first_token = true;
        self.send(UiEvent::ResponseComplete);
    }

    fn render_status(&mut self, _status: &str) {}

    fn render_thinking_start(&mut self) {
        self.first_token = true;
        self.streaming_text.clear();
        self.send(UiEvent::ThinkingStart);
    }

    fn render_thinking_stop(&mut self) {
        self.send(UiEvent::ThinkingStop);
    }
}
