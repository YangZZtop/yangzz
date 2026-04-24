//! Unified interactive wizard.
//!
//! Used by `/provider add`, `/mcp add`, `/skill add`, `yangzz --setup`, etc.
//! Gives every "add a thing" flow the SAME feel so users learn it once.
//!
//! ```rust,ignore
//! let answers = Wizard::new("添加新 provider")
//!     .ask("配置名", Some("my-relay-2"))
//!     .ask("入口地址", None)
//!     .ask_secret_optional("API Key（可留空）")
//!     .ask("默认模型", Some("gpt-4o"))
//!     .run();
//! ```

use crate::ui::format::*;
use std::io::{self, Write};

#[derive(Debug, Clone)]
pub enum QuestionKind {
    /// Plain text; optional default shown in brackets.
    Text { default: Option<String> },
    /// Secret (API key etc). Currently just plain read, but reserved for
    /// terminal no-echo in the future.
    Secret,
    /// Yes/no with optional default.
    #[allow(dead_code)]
    YesNo { default: bool },
}

#[derive(Debug, Clone)]
pub struct Question {
    pub label: String,
    pub kind: QuestionKind,
    /// If the raw answer is empty AND this is true, re-ask.
    pub required: bool,
}

impl Question {
    pub fn text(label: impl Into<String>, default: Option<&str>) -> Self {
        Self {
            label: label.into(),
            kind: QuestionKind::Text {
                default: default.map(String::from),
            },
            required: true,
        }
    }
    pub fn secret(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            kind: QuestionKind::Secret,
            required: true,
        }
    }
    #[allow(dead_code)]
    pub fn yes_no(label: impl Into<String>, default: bool) -> Self {
        Self {
            label: label.into(),
            kind: QuestionKind::YesNo { default },
            required: false,
        }
    }
    #[allow(dead_code)]
    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

pub struct Wizard {
    title: String,
    questions: Vec<Question>,
}

/// Result of running a wizard. `None` means the user cancelled (e.g. empty
/// input on a required field, or EOF).
pub type WizardResult = Option<Vec<String>>;

impl Wizard {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            questions: Vec::new(),
        }
    }

    pub fn ask(mut self, label: impl Into<String>, default: Option<&str>) -> Self {
        self.questions.push(Question::text(label, default));
        self
    }

    pub fn ask_secret(mut self, label: impl Into<String>) -> Self {
        self.questions.push(Question::secret(label));
        self
    }

    pub fn ask_secret_optional(mut self, label: impl Into<String>) -> Self {
        self.questions.push(Question::secret(label).optional());
        self
    }

    #[allow(dead_code)]
    pub fn ask_yes_no(mut self, label: impl Into<String>, default: bool) -> Self {
        self.questions.push(Question::yes_no(label, default));
        self
    }

    /// Run the wizard. Returns Some(answers) or None if cancelled.
    ///
    /// In TUI raw-mode the wizard cannot read stdin, so it emits a hint
    /// (captured by `with_capture`) and returns None.
    pub fn run(self) -> WizardResult {
        if crate::slash::output::is_tui_mode() {
            crate::slash::output::emit(&format!(
                "  ✖ 该操作需要交互输入，TUI 模式暂不支持。\n  → 用 yangzz --repl 再试 或直接编辑 config.toml"
            ));
            return None;
        }
        println!();
        println!(
            "  {BOLD_GOLD}{}{RESET} {DIM}（留空可取消）{RESET}",
            self.title
        );
        println!();

        let mut answers = Vec::with_capacity(self.questions.len());
        for q in &self.questions {
            match ask_one(q) {
                Some(v) => answers.push(v),
                None => {
                    println!("  {DIM}已取消。{RESET}");
                    return None;
                }
            }
        }
        Some(answers)
    }
}

fn ask_one(q: &Question) -> Option<String> {
    match &q.kind {
        QuestionKind::Text { default } => {
            let shown = match default {
                Some(d) => format!("  {BOLD}{}{RESET} {DIM}[{}]{RESET}: ", q.label, d),
                None => format!("  {BOLD}{}:{RESET} ", q.label),
            };
            let raw = read_line(&shown)?;
            if raw.is_empty() {
                match default {
                    Some(d) => Some(d.clone()),
                    None => {
                        if q.required {
                            None
                        } else {
                            Some(String::new())
                        }
                    }
                }
            } else {
                Some(raw)
            }
        }
        QuestionKind::Secret => {
            let raw = read_line(&format!("  {BOLD}{}:{RESET} ", q.label))?;
            if raw.is_empty() && q.required {
                None
            } else {
                Some(raw)
            }
        }
        QuestionKind::YesNo { default } => {
            let hint = if *default { "Y/n" } else { "y/N" };
            let raw = read_line(&format!(
                "  {BOLD}{}{RESET} {DIM}[{}]{RESET}: ",
                q.label, hint
            ))?;
            if raw.is_empty() {
                Some(if *default { "yes".into() } else { "no".into() })
            } else {
                let lower = raw.to_lowercase();
                Some(if lower.starts_with('y') {
                    "yes".into()
                } else {
                    "no".into()
                })
            }
        }
    }
}

fn read_line(prompt: &str) -> Option<String> {
    print!("{prompt}");
    io::stdout().flush().ok();
    let mut buf = String::new();
    if io::stdin().read_line(&mut buf).is_err() {
        return None;
    }
    Some(buf.trim().to_string())
}

/// Parse yes/no from a wizard answer string.
#[allow(dead_code)]
pub fn parse_yes(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower == "yes" || lower == "y"
}
