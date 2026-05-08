use crate::provider::StreamEvent;

/// Render trait — abstracts REPL vs TUI output
///
/// This is one of the 5 核心元. Both REPL and future TUI implement this trait.
pub trait Renderer: Send {
    /// Display a streaming text delta (partial response)
    fn render_text_delta(&mut self, text: &str);

    /// Notify that a tool is being called
    fn render_tool_start(&mut self, name: &str, id: &str);

    /// Notify that a tool is being called (with input for context display)
    fn render_tool_start_with_input(&mut self, name: &str, id: &str, input: &serde_json::Value) {
        let _ = input;
        self.render_tool_start(name, id);
    }

    /// Display tool execution result
    fn render_tool_result(&mut self, name: &str, result: &str, is_error: bool);

    /// Display an error message
    fn render_error(&mut self, message: &str);

    /// Display an info message
    fn render_info(&mut self, message: &str);

    /// Signal that the current response is complete
    fn render_complete(&mut self);

    /// Render the status line
    fn render_status(&mut self, status: &str);

    /// Show thinking/loading indicator
    fn render_thinking_start(&mut self) {}

    /// Hide thinking/loading indicator
    fn render_thinking_stop(&mut self) {}

    /// Display a reasoning/thinking delta (from DeepSeek, Kimi, etc.)
    fn render_thinking_delta(&mut self, _text: &str) {}

    /// Stop any active spinner (used on error paths)
    fn stop_spinner(&mut self) {
        self.render_thinking_stop();
    }

    /// Process a raw stream event
    fn on_stream_event(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::TextDelta { text } => {
                self.render_thinking_stop();
                self.render_text_delta(text);
            }
            StreamEvent::ThinkingDelta { text } => {
                self.render_thinking_stop();
                self.render_thinking_delta(text);
            }
            StreamEvent::ToolUseStart { .. } => {
                self.render_thinking_stop();
            }
            StreamEvent::MessageStop => self.render_complete(),
            StreamEvent::Error { message } => {
                self.render_thinking_stop();
                self.render_error(message);
            }
            _ => {}
        }
    }
}

/// Silent renderer for sub-agents — collects text output without terminal I/O
pub struct NullRenderer {
    pub collected_text: String,
}

impl NullRenderer {
    pub fn new() -> Self {
        Self {
            collected_text: String::new(),
        }
    }
}

impl Renderer for NullRenderer {
    fn render_text_delta(&mut self, text: &str) {
        self.collected_text.push_str(text);
    }
    fn render_tool_start(&mut self, _name: &str, _id: &str) {}
    fn render_tool_result(&mut self, _name: &str, _result: &str, _is_error: bool) {}
    fn render_error(&mut self, _message: &str) {}
    fn render_info(&mut self, _message: &str) {}
    fn render_complete(&mut self) {}
    fn render_status(&mut self, _status: &str) {}
}
