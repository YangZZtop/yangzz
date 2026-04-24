//! Meta slash commands that affect the REPL lifecycle.

use crate::emitln;
use crate::repl_help::print_help;
use crate::slash::{Category, CommandContext, Outcome, SlashCommand};
use crate::ui::format::*;

pub struct HelpCommand;
pub struct GuideCommand;
pub struct MigrateCommand;
pub struct QuitCommand;

impl SlashCommand for HelpCommand {
    fn name(&self) -> &'static str {
        "help"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["h", "?"]
    }
    fn category(&self) -> Category {
        Category::Meta
    }
    fn summary(&self) -> &'static str {
        "显示帮助"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /help\n\
         \x20 /help <topic>\n\
         \x20 /h\n\
         \x20 /?"
    }

    fn handle(&self, ctx: &mut CommandContext, args: &str) -> Outcome {
        print_help(ctx.skills, args);
        Outcome::Continue
    }
}

impl SlashCommand for GuideCommand {
    fn name(&self) -> &'static str {
        "guide"
    }
    fn category(&self) -> Category {
        Category::Meta
    }
    fn summary(&self) -> &'static str {
        "显示快速上手指南"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /guide"
    }

    fn handle(&self, _ctx: &mut CommandContext, _args: &str) -> Outcome {
        crate::print_guide();
        Outcome::Continue
    }
}

impl SlashCommand for MigrateCommand {
    fn name(&self) -> &'static str {
        "migrate"
    }
    fn category(&self) -> Category {
        Category::Meta
    }
    fn summary(&self) -> &'static str {
        "迁移其他 AI 工具的配置/记忆"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /migrate"
    }

    fn handle(&self, _ctx: &mut CommandContext, _args: &str) -> Outcome {
        let sources = yangzz_core::migrate::detect_sources();
        if sources.is_empty() {
            emitln!("  {DIM}No migration sources found (Claude Code, Codex CLI, Cursor){RESET}");
            return Outcome::Continue;
        }

        for source in &sources {
            emitln!(
                "  {GOLD}●{RESET} Found: {} ({} items)",
                source.tool.name(),
                source.items.len()
            );
        }

        let cwd = std::env::current_dir().unwrap_or_default();
        match yangzz_core::migrate::migrate_to_memory(&sources, &cwd) {
            Ok(results) => {
                for result in &results {
                    emitln!("  {result}");
                }
            }
            Err(err) => emitln!("  {RED}✖{RESET} Migration error: {err}"),
        }
        Outcome::Continue
    }
}

impl SlashCommand for QuitCommand {
    fn name(&self) -> &'static str {
        "quit"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["q", "exit"]
    }
    fn category(&self) -> Category {
        Category::Meta
    }
    fn summary(&self) -> &'static str {
        "退出 REPL（自动保存会话）"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /quit\n\
         \x20 /q\n\
         \x20 /exit"
    }

    fn handle(&self, _ctx: &mut CommandContext, _args: &str) -> Outcome {
        Outcome::Quit
    }
}

#[cfg(test)]
mod tests {
    use super::{GuideCommand, HelpCommand, MigrateCommand, QuitCommand};
    use crate::slash::{Category, SlashCommand};

    #[test]
    fn meta_commands_use_meta_category() {
        assert_eq!(HelpCommand.category(), Category::Meta);
        assert_eq!(GuideCommand.category(), Category::Meta);
        assert_eq!(MigrateCommand.category(), Category::Meta);
        assert_eq!(QuitCommand.category(), Category::Meta);
    }

    #[test]
    fn meta_command_aliases_match_expected_shortcuts() {
        assert_eq!(HelpCommand.aliases(), &["h", "?"]);
        assert_eq!(QuitCommand.aliases(), &["q", "exit"]);
    }
}
