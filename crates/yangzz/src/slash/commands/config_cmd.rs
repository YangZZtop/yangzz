//! /config — show current effective config (API keys masked).

use crate::emitln;

use crate::slash::{Category, CommandContext, Outcome, SlashCommand};
use crate::ui::format::*;
use yangzz_core::config::writer;

pub struct ConfigCommand;

impl SlashCommand for ConfigCommand {
    fn name(&self) -> &'static str {
        "config"
    }
    fn category(&self) -> Category {
        Category::Config
    }
    fn summary(&self) -> &'static str {
        "查看当前配置（key 脱敏）"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /config        显示全部解析后的配置\n\
         \x20 /config path   只打印 config.toml 路径"
    }

    fn handle(&self, _ctx: &mut CommandContext, args: &str) -> Outcome {
        let sub = args.trim();
        if sub == "path" {
            emitln!("  {}", yangzz_core::paths::config_path().display());
            return Outcome::Continue;
        }
        show();
        Outcome::Continue
    }
}

fn show() {
    let s = writer::load_raw_config();
    let path = yangzz_core::paths::config_path();

    emitln!();
    emitln!("  {BOLD}当前配置{RESET} {DIM}{}{RESET}", path.display());
    emitln!();

    // Top-level
    emitln!("  {BOLD}默认选择{RESET}");
    emitln!("    provider         = {}", show_opt(&s.provider));
    emitln!("    model            = {}", show_opt(&s.model));
    if let Some(v) = s.max_tokens {
        emitln!("    max_tokens       = {v}");
    }
    if let Some(v) = s.temperature {
        emitln!("    temperature      = {v}");
    }
    if let Some(v) = s.thinking_budget {
        emitln!("    thinking_budget  = {v}");
    }
    if let Some(v) = s.context_window {
        emitln!("    context_window   = {v}");
    }
    if let Some(ref v) = s.reasoning_effort {
        emitln!("    reasoning_effort = {v}");
    }
    emitln!();

    // Providers
    emitln!(
        "  {BOLD}Providers{RESET} {DIM}({}){RESET}",
        s.providers.len()
    );
    if s.providers.is_empty() {
        emitln!("    {DIM}(无) — /provider add 来添加{RESET}");
    } else {
        for p in &s.providers {
            emitln!();
            emitln!("    {BOLD}[{}]{RESET}", p.name);
            emitln!("      api_key       = {DIM}{}{RESET}", mask(&p.api_key));
            emitln!("      base_url      = {DIM}{}{RESET}", p.base_url);
            emitln!(
                "      default_model = {DIM}{}{RESET}",
                p.default_model.as_deref().unwrap_or("(none)")
            );
            emitln!(
                "      api_format    = {DIM}{}{RESET}",
                p.api_format.as_deref().unwrap_or("openai")
            );
        }
    }
    emitln!();
}

fn show_opt(o: &Option<String>) -> String {
    match o {
        Some(v) => v.clone(),
        None => format!("{DIM}(unset){RESET}"),
    }
}

fn mask(k: &str) -> String {
    if k.len() <= 8 {
        return "(hidden)".into();
    }
    let tail = &k[k.len().saturating_sub(4)..];
    format!("****{tail}")
}
