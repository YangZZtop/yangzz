use std::any::Any;
use std::process::Command;

/// Data returned by a segment
pub struct SegmentData {
    pub primary: String,
    pub secondary: String,
}

/// Segment trait — each segment produces a piece of the status line
pub trait Segment: Send {
    fn collect(&self) -> Option<SegmentData>;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

// --- Model Segment ---

pub struct ModelSegment {
    model: String,
    provider: String,
}

impl ModelSegment {
    pub fn new(model: &str, provider: &str) -> Self {
        Self {
            model: model.to_string(),
            provider: provider.to_string(),
        }
    }
}

impl Segment for ModelSegment {
    fn collect(&self) -> Option<SegmentData> {
        Some(SegmentData {
            primary: format!("🤖 {}", self.model),
            secondary: self.provider.clone(),
        })
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// --- Git Segment ---

pub struct GitSegment;

impl Segment for GitSegment {
    fn collect(&self) -> Option<SegmentData> {
        let branch = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())?;

        // Get short status
        let dirty = Command::new("git")
            .args(["status", "--porcelain"])
            .output()
            .ok()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);

        let marker = if dirty { "*" } else { "" };

        Some(SegmentData {
            primary: format!("⎇ {branch}{marker}"),
            secondary: String::new(),
        })
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// --- Context Window Segment ---

pub struct ContextSegment {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_turns: u32,
}

impl Default for ContextSegment {
    fn default() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            total_turns: 0,
        }
    }
}

impl ContextSegment {
    fn format_tokens(n: u32) -> String {
        if n >= 1_000_000 {
            format!("{:.1}M", n as f64 / 1_000_000.0)
        } else if n >= 1_000 {
            format!("{:.1}k", n as f64 / 1_000.0)
        } else {
            format!("{n}")
        }
    }
}

impl Segment for ContextSegment {
    fn collect(&self) -> Option<SegmentData> {
        if self.total_turns == 0 {
            return None;
        }

        let total = self.input_tokens + self.output_tokens;
        Some(SegmentData {
            primary: format!("📊 {}", Self::format_tokens(total)),
            secondary: format!(
                "{}↑ {}↓ T{}",
                Self::format_tokens(self.input_tokens),
                Self::format_tokens(self.output_tokens),
                self.total_turns,
            ),
        })
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
