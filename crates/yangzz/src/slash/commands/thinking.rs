//! /thinking — control reasoning/thinking depth at runtime.

use crate::emitln;
use crate::slash::{Category, CommandContext, Outcome, SlashCommand};
use crate::ui::format::*;
use yangzz_core::config::settings;

pub struct ThinkingCommand;

impl SlashCommand for ThinkingCommand {
    fn name(&self) -> &'static str {
        "thinking"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["think", "reasoning"]
    }
    fn category(&self) -> Category {
        Category::Config
    }
    fn summary(&self) -> &'static str {
        "设置思考深度 (off/low/medium/high/max)"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /thinking              查看当前设置\n\
         \x20 /thinking off          关闭思考（最快、最便宜）\n\
         \x20 /thinking low          轻度思考\n\
         \x20 /thinking medium       中度思考（默认）\n\
         \x20 /thinking high         深度思考（最强推理）\n\
         \x20 /thinking max          最大思考预算（128K tokens）\n\
         \x20 /thinking <number>     自定义 token 预算（如 50000）\n\n\
         思考深度越高，推理能力越强，但速度越慢、费用越高。\n\
         支持思考的模型：Claude Opus/Sonnet 4+, GPT-5.4, o3/o4, DeepSeek R1, QwQ"
    }

    fn handle(&self, _ctx: &mut CommandContext, args: &str) -> Outcome {
        let arg = args.trim().to_lowercase();

        if arg.is_empty() {
            // Show current state
            let budget = settings::get_runtime_thinking_budget();
            let effort = settings::get_runtime_reasoning_effort();
            emitln!();
            emitln!("  {BOLD}Thinking / Reasoning 设置{RESET}");
            emitln!();
            match (&effort, budget) {
                (Some(e), Some(b)) => {
                    emitln!("    {GOLD}Effort:{RESET}  {BOLD}{e}{RESET}");
                    emitln!("    {GOLD}Budget:{RESET}  {BOLD}{b}{RESET} tokens");
                }
                (Some(e), None) => {
                    emitln!("    {GOLD}Effort:{RESET}  {BOLD}{e}{RESET}");
                    emitln!("    {GOLD}Budget:{RESET}  {DIM}(auto){RESET}");
                }
                (None, Some(b)) => {
                    emitln!("    {GOLD}Effort:{RESET}  {DIM}(not set){RESET}");
                    emitln!("    {GOLD}Budget:{RESET}  {BOLD}{b}{RESET} tokens");
                }
                (None, None) => {
                    emitln!("    {DIM}未设置（使用模型默认行为）{RESET}");
                }
            }
            emitln!();
            emitln!("  {DIM}用法: /thinking off|low|medium|high|max|<number>{RESET}");
            emitln!();
            return Outcome::Continue;
        }

        match arg.as_str() {
            "off" | "none" | "0" | "关" => {
                settings::set_runtime_thinking(Some(0), Some("off".to_string()));
                emitln!("  {GREEN}✓{RESET} Thinking: {BOLD}OFF{RESET} {DIM}(不使用思考，最快){RESET}");
            }
            "low" | "l" | "低" => {
                settings::set_runtime_thinking(Some(8000), Some("low".to_string()));
                emitln!("  {GREEN}✓{RESET} Thinking: {BOLD}low{RESET} {DIM}(8K tokens 预算){RESET}");
            }
            "medium" | "med" | "m" | "中" => {
                settings::set_runtime_thinking(Some(32000), Some("medium".to_string()));
                emitln!("  {GREEN}✓{RESET} Thinking: {BOLD}medium{RESET} {DIM}(32K tokens 预算){RESET}");
            }
            "high" | "h" | "高" => {
                settings::set_runtime_thinking(Some(64000), Some("high".to_string()));
                emitln!("  {GREEN}✓{RESET} Thinking: {BOLD}high{RESET} {DIM}(64K tokens 预算){RESET}");
            }
            "max" | "最大" | "full" => {
                settings::set_runtime_thinking(Some(128000), Some("high".to_string()));
                emitln!("  {GREEN}✓{RESET} Thinking: {BOLD}MAX{RESET} {DIM}(128K tokens 预算){RESET}");
            }
            "auto" | "default" | "默认" => {
                settings::set_runtime_thinking(None, None);
                emitln!("  {GREEN}✓{RESET} Thinking: {BOLD}auto{RESET} {DIM}(使用模型默认行为){RESET}");
            }
            other => {
                // Try to parse as a number (custom budget)
                if let Ok(n) = other.parse::<u32>() {
                    let effort = if n == 0 {
                        "off"
                    } else if n <= 10000 {
                        "low"
                    } else if n <= 40000 {
                        "medium"
                    } else {
                        "high"
                    };
                    settings::set_runtime_thinking(Some(n), Some(effort.to_string()));
                    emitln!("  {GREEN}✓{RESET} Thinking budget: {BOLD}{n}{RESET} tokens {DIM}(effort: {effort}){RESET}");
                } else {
                    emitln!("  {RED}✖{RESET} 无效参数。用法: /thinking off|low|medium|high|max|<number>");
                }
            }
        }

        Outcome::Continue
    }
}
