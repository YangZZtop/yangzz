use crate::emitln;
use crate::slash::{Category, CommandContext, Outcome, SlashCommand, block_on};
use crate::ui::format::*;
use crate::ui::i18n::t;
use crate::ui::status;
use yangzz_core::session::Session;

pub struct ClearCommand;
pub struct StatusCommand;
pub struct UndoCommand;
pub struct CompactCommand;
pub struct MemoryCommand;
pub struct RecallCommand;
pub struct ResumeCommand;
pub struct HistoryCommand;

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
        // Show detailed stats including context window usage
        emitln!();
        emitln!("  {BOLD}Session Stats{RESET}");
        emitln!("    {GOLD}Model:{RESET}    {} via {}", ctx.stats.model, ctx.stats.provider);
        emitln!("    {GOLD}Turns:{RESET}    {}", ctx.stats.total_turns);
        emitln!("    {GOLD}Tokens:{RESET}   {} in / {} out (total: {})",
            format_tokens(ctx.stats.total_input_tokens),
            format_tokens(ctx.stats.total_output_tokens),
            format_tokens(ctx.stats.total_input_tokens + ctx.stats.total_output_tokens));
        emitln!("    {GOLD}Cost:{RESET}     ${:.4}", ctx.stats.total_cost_usd);
        emitln!("    {GOLD}Messages:{RESET} {}", ctx.messages.len());
        // Context window usage
        if ctx.stats.context_used > 0 {
            let ratio = ctx.stats.context_ratio();
            let pct = (ratio * 100.0) as u32;
            let ctx_max = yangzz_core::config::model_meta::format_context(ctx.stats.context_window);
            let level = yangzz_core::memory::MemoryLevel::from_usage(ratio);
            let color = if ratio < 0.50 { GREEN } else if ratio < 0.80 { GOLD } else { RED };
            emitln!("    {GOLD}Context:{RESET}  {color}{}{RESET} / {} ({color}{pct}%{RESET}) — Memory: {}",
                format_tokens(ctx.stats.context_used), ctx_max, level.label());
        } else {
            let ctx_max = yangzz_core::config::model_meta::format_context(ctx.stats.context_window);
            emitln!("    {GOLD}Context:{RESET}  0 / {} (0%)", ctx_max);
        }
        emitln!();
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

        // Try FTS5 search via SQLite first
        let db_path = yangzz_core::db::Database::default_path();
        if let Ok(db) = yangzz_core::db::Database::open(&db_path) {
            let project = std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if let Ok(memories) = db.search_memories(keyword, &project, 10) {
                if !memories.is_empty() {
                    emitln!("  {BOLD}Memories ({}):{RESET}", memories.len());
                    for m in &memories {
                        emitln!("  {DIM}[{}]{RESET} {}", m.kind, m.content.chars().take(80).collect::<String>());
                    }
                    emitln!();
                }
            }
        }

        // Also search JSON sessions
        let results = yangzz_core::session::Session::search(keyword);
        if results.is_empty() {
            emitln!("  {DIM}No session matches for \"{keyword}\"{RESET}");
            return Outcome::Continue;
        }

        emitln!("  {BOLD}Sessions ({}):{RESET}", results.len());
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

impl SlashCommand for ResumeCommand {
    fn name(&self) -> &'static str {
        "resume"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["r"]
    }
    fn category(&self) -> Category {
        Category::Conversation
    }
    fn summary(&self) -> &'static str {
        "恢复上次会话"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /resume       — 恢复当前目录最近的会话\n\
         \x20 /resume <id>  — 恢复指定 ID 的会话\n\
         \x20 /r"
    }

    fn handle(&self, ctx: &mut CommandContext, args: &str) -> Outcome {
        let session = if args.trim().is_empty() {
            Session::load_latest()
        } else {
            Session::load(args.trim())
        };

        match session {
            Some(s) if !s.messages.is_empty() => {
                let count = s.messages.len();
                *ctx.messages = s.messages;
                emitln!("  {GREEN}✓{RESET} Resumed session ({count} messages, model: {GOLD}{}{RESET})", s.model);
            }
            Some(_) => {
                emitln!("  {DIM}Session found but has no messages{RESET}");
            }
            None => {
                emitln!("  {RED}✖{RESET} No session found");
            }
        }
        Outcome::Continue
    }
}

impl SlashCommand for HistoryCommand {
    fn name(&self) -> &'static str {
        "history"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["sessions"]
    }
    fn category(&self) -> Category {
        Category::Conversation
    }
    fn summary(&self) -> &'static str {
        "列出最近会话"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /history      — 列出最近 10 个会话\n\
         \x20 /sessions"
    }

    fn handle(&self, _ctx: &mut CommandContext, _args: &str) -> Outcome {
        let sessions = Session::list_recent(10);
        if sessions.is_empty() {
            emitln!("  {DIM}No saved sessions{RESET}");
            return Outcome::Continue;
        }

        emitln!("  {BOLD}Recent sessions:{RESET}");
        for s in &sessions {
            let date = s.updated_at.get(..16).unwrap_or(&s.updated_at);
            let cwd_marker = if s.same_cwd { GOLD } else { DIM };
            let short_id = s.id.get(..8).unwrap_or(&s.id);
            emitln!(
                "  {cwd_marker}{short_id}{RESET}  {DIM}{date}{RESET}  {}{RESET}  ({} msgs)",
                s.model,
                s.message_count
            );
        }
        emitln!("  {DIM}Use /resume <id> to restore{RESET}");
        Outcome::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ClearCommand, CompactCommand, HistoryCommand, MemoryCommand, RecallCommand,
        ResumeCommand, StatusCommand, UndoCommand,
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
        assert_eq!(ResumeCommand.category(), Category::Conversation);
        assert_eq!(HistoryCommand.category(), Category::Conversation);
    }

    #[test]
    fn conversation_command_aliases_match_expected_shortcuts() {
        assert_eq!(ClearCommand.aliases(), &["c"]);
        assert_eq!(UndoCommand.aliases(), &["u"]);
        assert_eq!(RecallCommand.aliases(), &["search"]);
        assert_eq!(ResumeCommand.aliases(), &["r"]);
        assert_eq!(HistoryCommand.aliases(), &["sessions"]);
    }
}
