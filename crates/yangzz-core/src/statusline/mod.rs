mod segments;

pub use segments::{ContextSegment, GitSegment, ModelSegment, Segment, SegmentData};

/// StatusLine generator — collects segments and renders a status bar
pub struct StatusLine {
    segments: Vec<Box<dyn Segment>>,
    separator: String,
}

impl StatusLine {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            separator: " │ ".to_string(),
        }
    }

    pub fn with_defaults(model: &str, provider: &str) -> Self {
        let mut sl = Self::new();
        sl.add(Box::new(ModelSegment::new(model, provider)));
        sl.add(Box::new(GitSegment));
        sl.add(Box::new(ContextSegment::default()));
        sl
    }

    pub fn add(&mut self, segment: Box<dyn Segment>) {
        self.segments.push(segment);
    }

    /// Render the status line string
    pub fn render(&self) -> String {
        let parts: Vec<String> = self
            .segments
            .iter()
            .filter_map(|s| s.collect())
            .map(|data| {
                if data.secondary.is_empty() {
                    data.primary
                } else {
                    format!("{} {}", data.primary, data.secondary)
                }
            })
            .collect();

        parts.join(&self.separator)
    }

    /// Update context segment with token usage
    pub fn update_context(&mut self, input_tokens: u32, output_tokens: u32) {
        self.update_context_with_model(input_tokens, output_tokens, None);
    }

    /// Update context segment with token usage and model-based cost calculation
    pub fn update_context_with_model(&mut self, input_tokens: u32, output_tokens: u32, model: Option<&str>) {
        for seg in &mut self.segments {
            if let Some(ctx) = seg.as_any_mut().downcast_mut::<ContextSegment>() {
                ctx.input_tokens += input_tokens;
                ctx.output_tokens += output_tokens;
                ctx.total_turns += 1;

                if let Some(model_name) = model {
                    if let Some(meta) = crate::config::model_meta::lookup_model(model_name) {
                        let input_cost = (input_tokens as f64 / 1_000_000.0) * meta.input_price;
                        let output_cost = (output_tokens as f64 / 1_000_000.0) * meta.output_price;
                        ctx.cost_usd += input_cost + output_cost;
                    }
                }
            }
        }
    }
}

impl Default for StatusLine {
    fn default() -> Self {
        Self::new()
    }
}
