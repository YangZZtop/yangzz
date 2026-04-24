//! /tool — inspect available tools (built-in + MCP + plugins).

use crate::emitln;
use crate::slash::{Category, CommandContext, Outcome, SlashCommand};
use crate::ui::format::*;
use crate::ui::i18n::translate_tool_desc;

pub struct ToolCommand;

impl SlashCommand for ToolCommand {
    fn name(&self) -> &'static str {
        "tool"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["tools", "t"]
    }
    fn category(&self) -> Category {
        Category::Extension
    }
    fn summary(&self) -> &'static str {
        "列出可用工具"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /tool            列出全部（内置 + MCP + 插件）\n\
         \x20 /tools           同上\n\
         \x20 /t               同上\n\
         \x20 /tool list       同上"
    }

    fn handle(&self, ctx: &mut CommandContext, args: &str) -> Outcome {
        let sub = args.trim();
        if !sub.is_empty() && !matches!(sub, "list" | "ls") {
            emitln!("  {RED}✖{RESET} 未知子命令。/tool 查看用法。");
            return Outcome::Continue;
        }

        emitln!();
        emitln!("  {BOLD}工具{RESET}");
        for td in ctx.executor.tool_definitions() {
            let desc = translate_tool_desc(&td.name, &td.description);
            emitln!("    {BOLD_YELLOW}{:<14}{RESET} {DIM}{desc}{RESET}", td.name);
        }
        emitln!();
        Outcome::Continue
    }
}
