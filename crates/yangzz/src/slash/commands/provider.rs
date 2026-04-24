//! /provider + /model — provider / model switching and management.

use crate::emitln;

use crate::slash::wizard::Wizard;
use crate::slash::{Category, CommandContext, Outcome, SlashCommand};
use crate::ui::format::*;
use yangzz_core::config;
use yangzz_core::config::Settings;
use yangzz_core::config::settings::CliOverrides;
use yangzz_core::config::settings::ExtraProvider;
use yangzz_core::config::writer;

pub struct ProviderCommand;
pub struct ModelCommand;

impl SlashCommand for ModelCommand {
    fn name(&self) -> &'static str {
        "model"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["m"]
    }
    fn category(&self) -> Category {
        Category::Config
    }
    fn summary(&self) -> &'static str {
        "查看 / 切换当前模型"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /model\n\
         \x20 /m\n\
         \x20 /model <name>\n\
         \x20 /model <name> <provider>\n\n\
         REPL 中直接输入 /model 可打开模型选择器。"
    }

    fn handle(&self, ctx: &mut CommandContext, args: &str) -> Outcome {
        let args = args.trim();
        if args.is_empty() {
            show_current_model(ctx);
        } else {
            switch_model(args, ctx);
        }
        Outcome::Continue
    }
}

impl SlashCommand for ProviderCommand {
    fn name(&self) -> &'static str {
        "provider"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["p"]
    }
    fn category(&self) -> Category {
        Category::Config
    }
    fn summary(&self) -> &'static str {
        "查看 / 切换 / 管理 provider"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /provider                      显示当前 + 用法\n\
         \x20 /provider list                 列出全部\n\
         \x20 /provider add                  交互式添加\n\
         \x20 /provider remove <name>        删除\n\
         \x20 /provider rename <old> <new>   改名\n\
         \x20 /provider edit <name>          交互式改字段\n\
         \x20 /provider <name>               切到该 provider"
    }

    fn handle(&self, ctx: &mut CommandContext, args: &str) -> Outcome {
        let mut parts = args.splitn(2, char::is_whitespace);
        let sub = parts.next().unwrap_or("").trim();
        let rest = parts.next().unwrap_or("").trim();

        match sub {
            "" => show_current_and_usage(ctx),
            "list" | "ls" => list_providers(ctx),
            "add" => add_provider(),
            "remove" | "rm" | "delete" => {
                if rest.is_empty() {
                    emitln!("  {RED}✖{RESET} 用法：/provider remove <name>");
                } else {
                    remove_provider(rest, ctx);
                }
            }
            "rename" | "mv" => {
                let mut names = rest.splitn(2, char::is_whitespace);
                let old = names.next().unwrap_or("").trim();
                let new = names.next().unwrap_or("").trim();
                if old.is_empty() || new.is_empty() {
                    emitln!("  {RED}✖{RESET} 用法：/provider rename <old> <new>");
                } else {
                    rename_provider(old, new, ctx);
                }
            }
            "edit" => {
                if rest.is_empty() {
                    emitln!("  {RED}✖{RESET} 用法：/provider edit <name>");
                } else {
                    edit_provider(rest);
                }
            }
            other => switch_provider(other, ctx),
        }
        Outcome::Continue
    }
}

fn show_current_model(ctx: &CommandContext) {
    emitln!();
    emitln!("  {DIM}当前模型:{RESET} {BOLD}{}{RESET}", ctx.current_model);
    emitln!(
        "  {DIM}当前 provider:{RESET} {GOLD}{}{RESET}",
        ctx.current_provider.name()
    );
    emitln!("  {DIM}切换: /model <name> [provider]{RESET}");
    emitln!("  {DIM}REPL 下直接 /model 可打开模型选择器{RESET}");
    emitln!();
}

fn show_current_and_usage(ctx: &CommandContext) {
    emitln!();
    emitln!(
        "  {DIM}当前:{RESET} {BOLD}{}{RESET}",
        ctx.current_provider.name()
    );
    emitln!();
    emitln!("  {BOLD}可用操作：{RESET}");
    emitln!("    {GOLD}/provider list{RESET}              {DIM}列出全部{RESET}");
    emitln!("    {GOLD}/provider add{RESET}               {DIM}添加新的（交互式）{RESET}");
    emitln!("    {GOLD}/provider edit <name>{RESET}       {DIM}改字段{RESET}");
    emitln!("    {GOLD}/provider rename <old> <new>{RESET} {DIM}改名{RESET}");
    emitln!("    {GOLD}/provider remove <name>{RESET}     {DIM}删除{RESET}");
    emitln!("    {GOLD}/provider <name>{RESET}            {DIM}切到该 provider{RESET}");
    emitln!();
}

fn list_providers(ctx: &CommandContext) {
    let s = writer::load_raw_config();
    if s.providers.is_empty() {
        emitln!("  {DIM}(还没有任何 provider。试试 /provider add){RESET}");
        return;
    }
    let active = ctx.current_provider.name();
    emitln!();
    emitln!(
        "  {BOLD}已配置的 provider{RESET} {DIM}({} 个){RESET}",
        s.providers.len()
    );
    emitln!();
    for p in &s.providers {
        let marker = if p.name.eq_ignore_ascii_case(active) {
            format!("{BOLD_GOLD}● 当前{RESET}")
        } else {
            format!("{DIM}      {RESET}")
        };
        let model = p.default_model.as_deref().unwrap_or("(none)");
        let fmt = p.api_format.as_deref().unwrap_or("openai");
        emitln!(
            "  {marker}  {BOLD}{:<16}{RESET}  {DIM}{}{RESET}",
            p.name,
            p.base_url
        );
        emitln!(
            "           {DIM}model: {}  ·  format: {}{RESET}",
            model,
            fmt
        );
    }
    emitln!();
}

fn switch_model(args: &str, ctx: &mut CommandContext) {
    let mut parts = args.split_whitespace();
    let Some(new_model) = parts.next().filter(|value| !value.is_empty()) else {
        emitln!("  {RED}✖{RESET} 用法：/model <name> [provider]");
        return;
    };

    let provider_name = parts
        .next()
        .map(str::to_string)
        .or_else(|| {
            config::select_provider_name_for_model(
                ctx.settings,
                new_model,
                Some(ctx.current_provider.name()),
            )
        })
        .or_else(|| config::detect_provider_family(new_model).map(str::to_string))
        .unwrap_or_else(|| ctx.current_provider.name().to_string());

    let mut settings = ctx.settings.clone();
    settings.model = Some(new_model.to_string());
    settings.provider = Some(provider_name);

    match config::resolve_provider(&settings) {
        Ok(new_provider) => {
            let old_provider = ctx.current_provider.name().to_string();
            *ctx.current_model = new_model.to_string();
            *ctx.current_provider = new_provider;
            ctx.stats.model = ctx.current_model.clone();
            ctx.stats.provider = ctx.current_provider.name().to_string();

            if old_provider != ctx.current_provider.name() {
                emitln!(
                    "  {GREEN}●{RESET} {BOLD}{}{RESET} {DIM}via{RESET} {GOLD}{}{RESET}",
                    ctx.current_model,
                    ctx.current_provider.name()
                );
            } else {
                emitln!("  {GREEN}●{RESET} {BOLD}{}{RESET}", ctx.current_model);
            }
        }
        Err(err) => emitln!("  {RED}✖{RESET} Cannot switch: {err}"),
    }
}

fn add_provider() {
    let Some(answers) = Wizard::new("添加新 provider")
        .ask("配置名（唯一）", Some("my-relay-2"))
        .ask("入口地址（含路径）", None)
        .ask_secret_optional("API Key（可留空）")
        .ask("默认模型", Some("claude-sonnet-4-20250514"))
        .run()
    else {
        return;
    };

    let provider = ExtraProvider {
        name: answers[0].clone(),
        base_url: answers[1].clone(),
        api_key: answers[2].clone(),
        default_model: Some(answers[3].clone()),
        api_format: Some("openai".to_string()),
        thinking_budget: None,
        context_window: None,
        reasoning_effort: None,
        max_tokens: None,
        temperature: None,
    };

    match writer::add_provider(provider) {
        Ok(path) => {
            emitln!();
            emitln!("  {GREEN}✓{RESET} 已添加 {BOLD}{}{RESET}", answers[0]);
            emitln!("  {DIM}写入: {}{RESET}", path.display());
            emitln!("  {DIM}→ /provider {} 切到它{RESET}", answers[0]);
            emitln!();
        }
        Err(e) => {
            emitln!("  {RED}✖{RESET} 添加失败: {e}");
        }
    }
}

fn remove_provider(name: &str, ctx: &CommandContext) {
    if name.eq_ignore_ascii_case(ctx.current_provider.name()) {
        emitln!("  {RED}✖{RESET} 不能删除正在使用的 provider。先 /provider <另一个> 切走。");
        return;
    }
    match writer::remove_provider(name) {
        Ok(_) => emitln!("  {GREEN}✓{RESET} 已删除 {BOLD}{name}{RESET}"),
        Err(e) => emitln!("  {RED}✖{RESET} {e}"),
    }
}

fn rename_provider(old: &str, new: &str, ctx: &CommandContext) {
    match writer::rename_provider(old, new) {
        Ok(_) => {
            emitln!("  {GREEN}✓{RESET} {BOLD}{old}{RESET} → {BOLD}{new}{RESET}");
            if old.eq_ignore_ascii_case(ctx.current_provider.name()) {
                emitln!("  {DIM}（下次启动或 /provider {new} 后生效）{RESET}");
            }
        }
        Err(e) => emitln!("  {RED}✖{RESET} {e}"),
    }
}

fn edit_provider(name: &str) {
    let s = writer::load_raw_config();
    let Some(p) = s
        .providers
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
    else {
        emitln!("  {RED}✖{RESET} Provider '{name}' 不存在");
        return;
    };

    emitln!();
    emitln!("  {BOLD}编辑 {BOLD_GOLD}{}{RESET}", p.name);
    emitln!();
    emitln!("  {DIM}当前值：{RESET}");
    emitln!("    api_key       = {DIM}{}{RESET}", mask_key(&p.api_key));
    emitln!("    base_url      = {DIM}{}{RESET}", p.base_url);
    emitln!(
        "    default_model = {DIM}{}{RESET}",
        p.default_model.as_deref().unwrap_or("(none)")
    );
    emitln!(
        "    api_format    = {DIM}{}{RESET}",
        p.api_format.as_deref().unwrap_or("openai")
    );
    emitln!();
    emitln!("  {DIM}可改字段：api_key / base_url / default_model / api_format /{RESET}");
    emitln!("  {DIM}          max_tokens / thinking_budget / context_window /{RESET}");
    emitln!("  {DIM}          reasoning_effort / temperature{RESET}");
    emitln!();

    let Some(answers) = Wizard::new("改哪个字段？")
        .ask("字段名", None)
        .ask("新值", None)
        .run()
    else {
        return;
    };

    match writer::edit_provider_field(name, &answers[0], &answers[1]) {
        Ok(_) => emitln!(
            "  {GREEN}✓{RESET} 已更新 {BOLD}{name}.{}{RESET}",
            answers[0]
        ),
        Err(e) => emitln!("  {RED}✖{RESET} {e}"),
    }
}

fn switch_provider(name: &str, ctx: &mut CommandContext) {
    let mut settings = Settings::load(CliOverrides::default());
    settings.provider = Some(name.to_string());
    settings.model = target_default_model_for_provider(&settings, name);
    match config::resolve_provider(&settings) {
        Ok(new_provider) => {
            *ctx.current_model = new_provider.default_model().to_string();
            *ctx.current_provider = new_provider;
            ctx.stats.model = ctx.current_model.clone();
            ctx.stats.provider = ctx.current_provider.name().to_string();
            emitln!(
                "  {GREEN}✓ 已切到{RESET} {BOLD}{}{RESET} {DIM}· {}{RESET}",
                ctx.current_provider.name(),
                ctx.current_model,
            );
        }
        Err(e) => emitln!("  {RED}✖{RESET} 无法切换: {e}"),
    }
}

fn target_default_model_for_provider(settings: &Settings, name: &str) -> Option<String> {
    if let Some(model) = settings
        .providers
        .iter()
        .find(|provider| provider.name.eq_ignore_ascii_case(name))
        .and_then(|provider| provider.default_model.clone())
    {
        return Some(model);
    }

    if config::PRESETS
        .iter()
        .any(|preset| preset.name.eq_ignore_ascii_case(name))
    {
        let mut probe = settings.clone();
        probe.provider = Some(name.to_string());
        probe.model = None;
        return Some(probe.resolved_model());
    }

    None
}

/// Mask an API key for display — show last 4 chars only.
fn mask_key(k: &str) -> String {
    if k.len() <= 8 {
        return "(hidden)".into();
    }
    let tail = &k[k.len().saturating_sub(4)..];
    format!("****{tail}")
}

#[cfg(test)]
mod tests {
    use super::{ModelCommand, ProviderCommand, target_default_model_for_provider};
    use crate::slash::{CommandContext, Outcome, Registry, SlashCommand};
    use crate::ui::status::SessionStats;
    use std::sync::Arc;
    use yangzz_core::config::settings::ExtraProvider;
    use yangzz_core::config::{self, Settings};
    use yangzz_core::message::Message;
    use yangzz_core::permission::PermissionManager;
    use yangzz_core::tool::{ToolExecutor, ToolRegistry};

    #[test]
    fn detect_provider_from_common_model_names() {
        assert_eq!(
            config::detect_provider_family("claude-sonnet-4"),
            Some("anthropic")
        );
        assert_eq!(config::detect_provider_family("gpt-4o"), Some("openai"));
        assert_eq!(
            config::detect_provider_family("gemini-2.5-pro"),
            Some("gemini")
        );
        assert_eq!(
            config::detect_provider_family("deepseek-chat"),
            Some("deepseek")
        );
        assert_eq!(config::detect_provider_family("qwen2.5"), Some("ollama"));
        assert_eq!(config::detect_provider_family("mystery-model"), None);
    }

    #[test]
    fn provider_command_aliases_match_expected_shortcuts() {
        assert_eq!(ModelCommand.aliases(), &["m"]);
        assert_eq!(ProviderCommand.aliases(), &["p"]);
    }

    #[test]
    fn model_command_dispatch_switches_to_explicit_provider() {
        let settings = Settings {
            provider: Some("relay-openai".into()),
            model: Some("gpt-4o-mini".into()),
            providers: vec![
                extra_provider("relay-openai", "gpt-4o-mini"),
                extra_provider("relay-two", "gpt-5.4"),
            ],
            ..Settings::default()
        };
        let mut current_model = settings.model.clone().unwrap();
        let mut current_provider = config::resolve_provider(&settings).unwrap();
        let mut messages = Vec::<Message>::new();
        let mut stats = SessionStats::new(&current_model, current_provider.name());
        let executor = ToolExecutor::new(
            ToolRegistry::new(),
            Arc::new(PermissionManager::auto_approve()),
            std::env::temp_dir(),
        );
        let mut ctx = CommandContext {
            current_model: &mut current_model,
            current_provider: &mut current_provider,
            messages: &mut messages,
            stats: &mut stats,
            settings: &settings,
            executor: &executor,
            skills: &[],
        };
        let mut registry = Registry::new();
        registry.register(Box::new(ModelCommand));

        let outcome = registry.dispatch(&mut ctx, "/m gpt-5.4 relay-two");

        assert_eq!(outcome, Outcome::Continue);
        assert_eq!(ctx.current_model.as_str(), "gpt-5.4");
        assert_eq!(ctx.current_provider.name(), "relay-two");
        assert_eq!(ctx.stats.model, "gpt-5.4");
        assert_eq!(ctx.stats.provider, "relay-two");
    }

    #[test]
    fn target_default_model_for_provider_prefers_custom_provider_default_model() {
        let settings = Settings {
            providers: vec![
                extra_provider("relay-openai", "gpt-4o-mini"),
                extra_provider("xiaomi", "mimo-v2.5-pro"),
            ],
            ..Settings::default()
        };

        let model = target_default_model_for_provider(&settings, "xiaomi");

        assert_eq!(model.as_deref(), Some("mimo-v2.5-pro"));
    }

    #[test]
    fn target_default_model_for_provider_supports_builtin_xiaomi_preset() {
        let settings = Settings::default();

        let model = target_default_model_for_provider(&settings, "xiaomi");

        assert_eq!(model.as_deref(), Some("mimo-v2.5-pro"));
    }

    fn extra_provider(name: &str, default_model: &str) -> ExtraProvider {
        ExtraProvider {
            name: name.into(),
            api_key: "sk-test".into(),
            base_url: "https://example.com/v1".into(),
            default_model: Some(default_model.into()),
            api_format: Some("openai".into()),
            thinking_budget: None,
            context_window: None,
            reasoning_effort: None,
            max_tokens: None,
            temperature: None,
        }
    }
}
