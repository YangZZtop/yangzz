use crate::provider::StreamEvent;

/// Render trait — abstracts REPL vs TUI output
///
/// This is one of the 5 核心元. Both REPL and future TUI implement this trait.
pub trait Renderer: Send {
    /// Display a streaming text delta (partial response)
    fn render_text_delta(&mut self, text: &str);

    /// Notify that a tool is being called
    fn render_tool_start(&mut self, name: &str, id: &str);

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

    /// Process a raw stream event
    fn on_stream_event(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::TextDelta { text } => {
                self.render_thinking_stop();
                self.render_text_delta(text);
            }
            StreamEvent::ToolUseStart { name, id } => {
                self.render_thinking_stop();
                self.render_tool_start(name, id);
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
