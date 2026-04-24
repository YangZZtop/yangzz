//! Slash command system.
//!
//! Design goals (from UX audit):
//! - **Noun-first**: `/provider add`, `/mcp list` (not `/add-provider`, `/list-mcp`)
//! - **Unified subcommand grammar**: every command supports `list`, some support
//!   `add`/`remove`/`rename`/`edit`; naming is consistent across commands.
//! - **Self-describing**: each command carries its own help, no central docs file
//!   that drifts out of sync.
//! - **CLI and REPL share the same handler**: `yangzz provider add` and
//!   `/provider add` both dispatch into this system.
//! - **Extensible**: adding a new command means writing one file that implements
//!   `SlashCommand` and registering it.

use std::future::Future;
use std::sync::Arc;
use yangzz_core::config::Settings;
use yangzz_core::message::Message;
use yangzz_core::provider::Provider;
use yangzz_core::skill::Skill;
use yangzz_core::tool::ToolExecutor;

use crate::ui::status::SessionStats;

pub mod commands;
pub mod output;
pub mod readline_helper;
pub mod wizard;

/// Run an async future from the sync slash-command layer.
pub fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => tokio::runtime::Runtime::new()
            .expect("create tokio runtime for slash command")
            .block_on(future),
    }
}

/// What happens after a command runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Stay in REPL, continue prompt loop.
    Continue,
    /// Quit the REPL.
    Quit,
    /// Not handled — try the legacy handler / treat as chat.
    Unhandled,
}

/// Logical grouping for `/help`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// Core configuration: provider, model, key, config
    Config,
    /// Extensions: mcp, skill, tool, plugin
    Extension,
    /// Conversation: clear, compact, undo, memory, session, recall, status
    Conversation,
    /// Tasks / routing / strategy / profile / policy
    Task,
    /// Meta: help, guide, migrate, quit
    Meta,
}

impl Category {
    pub fn title(self) -> &'static str {
        match self {
            Category::Config => "配置",
            Category::Extension => "扩展",
            Category::Conversation => "对话",
            Category::Task => "任务",
            Category::Meta => "其他",
        }
    }
}

/// Context passed into every command handler.
pub struct CommandContext<'a> {
    pub current_model: &'a mut String,
    pub current_provider: &'a mut Arc<dyn Provider>,
    pub messages: &'a mut Vec<Message>,
    pub stats: &'a mut SessionStats,
    pub settings: &'a Settings,
    pub executor: &'a ToolExecutor,
    pub skills: &'a [Skill],
}

/// A single slash command.
pub trait SlashCommand: Send + Sync {
    /// Primary name, e.g. "provider". NO leading slash.
    fn name(&self) -> &'static str;

    /// Aliases (no leading slash). E.g. ["p"] for "provider".
    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }

    /// Logical category for help grouping.
    fn category(&self) -> Category;

    /// One-line summary for help index.
    fn summary(&self) -> &'static str;

    /// Detailed help shown by `/help <name>`.
    fn detailed_help(&self) -> &'static str {
        ""
    }

    /// Handle the command. `args` is everything after the command name,
    /// trimmed. The command is responsible for further parsing (subcommand,
    /// positional args). Returns `Unhandled` only if it explicitly refuses
    /// (almost never).
    fn handle(&self, ctx: &mut CommandContext, args: &str) -> Outcome;
}

/// Registry: maps "/name" and "/alias" → command.
pub struct Registry {
    commands: Vec<Box<dyn SlashCommand>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn register(&mut self, cmd: Box<dyn SlashCommand>) {
        self.commands.push(cmd);
    }

    /// Look up by a bare name (no leading slash), case-insensitive.
    pub fn find(&self, name: &str) -> Option<&dyn SlashCommand> {
        let needle = name.trim_start_matches('/').to_lowercase();
        for c in &self.commands {
            if c.name().eq_ignore_ascii_case(&needle) {
                return Some(&**c);
            }
            for a in c.aliases() {
                if a.eq_ignore_ascii_case(&needle) {
                    return Some(&**c);
                }
            }
        }
        None
    }

    /// All commands in registration order.
    pub fn all(&self) -> &[Box<dyn SlashCommand>] {
        &self.commands
    }

    /// Group commands by category for `/help`.
    pub fn grouped(&self) -> Vec<(Category, Vec<&dyn SlashCommand>)> {
        let cats = [
            Category::Config,
            Category::Extension,
            Category::Conversation,
            Category::Task,
            Category::Meta,
        ];
        let mut out = Vec::new();
        for cat in cats {
            let mut in_cat: Vec<&dyn SlashCommand> = self
                .commands
                .iter()
                .filter(|c| c.category() == cat)
                .map(|c| &**c)
                .collect();
            in_cat.sort_by_key(|c| c.name());
            if !in_cat.is_empty() {
                out.push((cat, in_cat));
            }
        }
        out
    }

    /// Dispatch a line like "/provider add foo bar".
    /// Returns Unhandled if the command wasn't found in this registry.
    pub fn dispatch(&self, ctx: &mut CommandContext, line: &str) -> Outcome {
        let trimmed = line.trim();
        let without_slash = trimmed.trim_start_matches('/');
        let mut parts = without_slash.splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or("");
        let args = parts.next().unwrap_or("").trim();
        match self.find(name) {
            Some(cmd) => cmd.handle(ctx, args),
            None => Outcome::Unhandled,
        }
    }
}

/// Build the full v0.3.0 registry with all known commands.
pub fn build_default() -> Registry {
    let mut r = Registry::new();
    commands::register_all(&mut r);
    r
}
