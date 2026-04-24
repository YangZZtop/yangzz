use crate::emitln;
use crate::slash::{Category, CommandContext, Outcome, SlashCommand, block_on};
use crate::ui::format::*;
use crate::ui::i18n::t;
use crate::ui::status;

pub struct ClearCommand;
pub struct StatusCommand;
pub struct UndoCommand;
pub struct CompactCommand;
pub struct MemoryCommand;
pub struct RecallCommand;

impl SlashCommand for ClearCommand {
    fn name(&self) -> &'static str {
        "clear"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["c"]
    }
    fn category(&self) -> Category {
        Category::Conversation
    }
    fn summary(&self) -> &'static str {
        "清空对话历史"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /clear\n\
         \x20 /c"
    }

    fn handle(&self, ctx: &mut CommandContext, args: &str) -> Outcome {
        if !args.trim().is_empty() {
            emitln!("  {RED}✖{RESET} /clear 不接受参数");
            return Outcome::Continue;
        }
        ctx.messages.clear();
        emitln!("  {SOFT_GOLD}{}{RESET}", t().conversation_cleared);
        Outcome::Continue
    }
}

impl SlashCommand for StatusCommand {
    fn name(&self) -> &'static str {
        "status"
    }
    fn category(&self) -> Category {
        Category::Conversation
    }
    fn summary(&self) -> &'static str {
        "会话统计"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /status"
    }

    fn handle(&self, ctx: &mut CommandContext, args: &str) -> Outcome {
        if !args.trim().is_empty() {
            emitln!("  {RED}✖{RESET} /status 不接受参数");
            return Outcome::Continue;
        }
        status::emit_status_bar(ctx.stats);
        Outcome::Continue
    }
}

impl SlashCommand for UndoCommand {
    fn name(&self) -> &'static str {
        "undo"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["u"]
    }
    fn category(&self) -> Category {
        Category::Conversation
    }
    fn summary(&self) -> &'static str {
        "撤销上次文件修改"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /undo\n\
         \x20 /u"
    }

    fn handle(&self, ctx: &mut CommandContext, args: &str) -> Outcome {
        if !args.trim().is_empty() {
            emitln!("  {RED}✖{RESET} /undo 不接受参数");
            return Outcome::Continue;
        }
        let msg = block_on(ctx.executor.undo()).unwrap_or_else(|| "Nothing to undo".into());
        emitln!("  {GREEN}↩{RESET} {msg}");
        Outcome::Continue
    }
}

impl SlashCommand for CompactCommand {
    fn name(&self) -> &'static str {
        "compact"
    }
    fn category(&self) -> Category {
        Category::Conversation
    }
    fn summary(&self) -> &'static str {
        "压缩对话历史"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /compact"
    }

    fn handle(&self, ctx: &mut CommandContext, args: &str) -> Outcome {
        if !args.trim().is_empty() {
            emitln!("  {RED}✖{RESET} /compact 不接受参数");
            return Outcome::Continue;
        }
        let before = ctx.messages.len();
        yangzz_core::query::compact_messages_public(ctx.messages);
        let after = ctx.messages.len();
        emitln!("  {DIM}Compacted: {before} → {after} messages{RESET}");
        Outcome::Continue
    }
}

impl SlashCommand for MemoryCommand {
    fn name(&self) -> &'static str {
        "memory"
    }
    fn category(&self) -> Category {
        Category::Conversation
    }
    fn summary(&self) -> &'static str {
        "查看/追加 MEMORY.md"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /memory\n\
         \x20 /memory <text>"
    }

    fn handle(&self, _ctx: &mut CommandContext, args: &str) -> Outcome {
        let cwd = std::env::current_dir().unwrap_or_default();
        let arg = args.trim();
        if arg.is_empty() {
            match yangzz_core::memory::load_memory(&cwd) {
                Some(mem) => emitln!("  {DIM}{mem}{RESET}"),
                None => emitln!("  {DIM}No MEMORY.md found{RESET}"),
            }
        } else {
            match yangzz_core::memory::append_memory(&cwd, arg) {
                Ok(()) => emitln!("  {GREEN}✓{RESET} Saved to MEMORY.md"),
                Err(e) => emitln!("  {RED}✖{RESET} {e}"),
            }
        }
        Outcome::Continue
    }
}

impl SlashCommand for RecallCommand {
    fn name(&self) -> &'static str {
        "recall"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["search"]
    }
    fn category(&self) -> Category {
        Category::Conversation
    }
    fn summary(&self) -> &'static str {
        "搜索过去会话"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /recall <keyword>\n\
         \x20 /search <keyword>"
    }

    fn handle(&self, _ctx: &mut CommandContext, args: &str) -> Outcome {
        let keyword = args.trim();
        if keyword.is_empty() {
            emitln!("  {RED}✖{RESET} 用法：/recall <keyword>");
            return Outcome::Continue;
        }

        let results = yangzz_core::session::Session::search(keyword);
        if results.is_empty() {
            emitln!("  {DIM}No matches for \"{keyword}\"{RESET}");
            return Outcome::Continue;
        }

        emitln!("  {BOLD}Found {} results:{RESET}", results.len());
        for (idx, result) in results.iter().enumerate() {
            let date = result.date.get(..10).unwrap_or(&result.date);
            emitln!(
                "  {DIM}{date}{RESET} [{GOLD}{}{RESET}] ...{}...",
                result.model,
                result.snippet.replace('\n', " ").trim()
            );
            if idx >= 9 {
                break;
            }
        }
        Outcome::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ClearCommand, CompactCommand, MemoryCommand, RecallCommand, StatusCommand, UndoCommand,
    };
    use crate::slash::{Category, SlashCommand};

    #[test]
    fn conversation_commands_use_conversation_category() {
        assert_eq!(ClearCommand.category(), Category::Conversation);
        assert_eq!(StatusCommand.category(), Category::Conversation);
        assert_eq!(UndoCommand.category(), Category::Conversation);
        assert_eq!(CompactCommand.category(), Category::Conversation);
        assert_eq!(MemoryCommand.category(), Category::Conversation);
        assert_eq!(RecallCommand.category(), Category::Conversation);
    }

    #[test]
    fn conversation_command_aliases_match_expected_shortcuts() {
        assert_eq!(ClearCommand.aliases(), &["c"]);
        assert_eq!(UndoCommand.aliases(), &["u"]);
        assert_eq!(RecallCommand.aliases(), &["search"]);
    }
}
