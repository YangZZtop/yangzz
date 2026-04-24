//! All slash command implementations. Each file defines one command (or a
//! tightly related family). They're registered in `register_all` below.
//!
//! To add a new command: write a new file here implementing `SlashCommand`,
//! then add one `r.register(Box::new(...))` line.

use super::Registry;

mod config_cmd;
mod conversation;
mod key;
mod mcp;
mod meta;
pub(crate) mod provider;
mod skill;
mod task;
mod tool;

pub fn register_all(r: &mut Registry) {
    r.register(Box::new(provider::ModelCommand));
    r.register(Box::new(provider::ProviderCommand));
    r.register(Box::new(key::KeyCommand));
    r.register(Box::new(config_cmd::ConfigCommand));
    r.register(Box::new(conversation::ClearCommand));
    r.register(Box::new(conversation::StatusCommand));
    r.register(Box::new(conversation::UndoCommand));
    r.register(Box::new(conversation::CompactCommand));
    r.register(Box::new(conversation::MemoryCommand));
    r.register(Box::new(conversation::RecallCommand));
    r.register(Box::new(mcp::McpCommand));
    r.register(Box::new(task::TaskCommand));
    r.register(Box::new(task::RouteCommand));
    r.register(Box::new(task::StrategyCommand));
    r.register(Box::new(task::ProfileCommand));
    r.register(Box::new(task::PolicyCommand));
    r.register(Box::new(skill::SkillCommand));
    r.register(Box::new(skill::SkillsCommand));
    r.register(Box::new(tool::ToolCommand));
    r.register(Box::new(meta::HelpCommand));
    r.register(Box::new(meta::GuideCommand));
    r.register(Box::new(meta::MigrateCommand));
    r.register(Box::new(meta::QuitCommand));
}
