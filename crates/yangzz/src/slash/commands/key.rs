//! /key — quick API key rotation without needing /provider edit flow.

use crate::emitln;

use crate::slash::wizard::Wizard;
use crate::slash::{Category, CommandContext, Outcome, SlashCommand};
use crate::ui::format::*;
use yangzz_core::config::writer;

pub struct KeyCommand;

impl SlashCommand for KeyCommand {
    fn name(&self) -> &'static str {
        "key"
    }
    fn category(&self) -> Category {
        Category::Config
    }
    fn summary(&self) -> &'static str {
        "轮换 API Key"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /key                   列出所有 provider + 当前 key 的脱敏提示\n\
         \x20 /key <name>            交互式输入新 key\n\
         \x20 /key <name> <newkey>   直接指定新 key（脚本友好）"
    }

    fn handle(&self, _ctx: &mut CommandContext, args: &str) -> Outcome {
        let mut parts = args.splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or("").trim();
        let new_key_inline = parts.next().unwrap_or("").trim();

        if name.is_empty() {
            show_all_keys();
            return Outcome::Continue;
        }

        let new_key = if !new_key_inline.is_empty() {
            new_key_inline.to_string()
        } else {
            let Some(answers) = Wizard::new(&format!("为 {name} 设置新 Key"))
                .ask_secret("新 API Key")
                .run()
            else {
                return Outcome::Continue;
            };
            answers[0].clone()
        };

        match writer::rotate_api_key(name, &new_key) {
            Ok(_) => emitln!("  {GREEN}✓{RESET} 已更新 {BOLD}{name}{RESET} 的 API Key"),
            Err(e) => emitln!("  {RED}✖{RESET} {e}"),
        }
        Outcome::Continue
    }
}

fn show_all_keys() {
    let s = writer::load_raw_config();
    if s.providers.is_empty() {
        emitln!("  {DIM}(还没有 provider){RESET}");
        return;
    }
    emitln!();
    emitln!("  {BOLD}当前 API Keys（已脱敏）{RESET}");
    emitln!();
    for p in &s.providers {
        emitln!(
            "    {BOLD}{:<16}{RESET} {DIM}{}{RESET}",
            p.name,
            mask(&p.api_key)
        );
    }
    emitln!();
    emitln!("  {DIM}轮换：/key <name> 或 /key <name> <newkey>{RESET}");
    emitln!();
}

fn mask(k: &str) -> String {
    if k.len() <= 8 {
        return "(hidden)".into();
    }
    let tail = &k[k.len().saturating_sub(4)..];
    format!("****{tail}")
}
