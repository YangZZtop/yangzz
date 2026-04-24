use std::sync::Arc;

use crate::slash::{self, CommandContext as SlashCtx, Outcome as SlashOutcome};
use crate::ui::format::*;
use yangzz_core::config;
use yangzz_core::config::Settings;
use yangzz_core::config::settings::CliOverrides;
use yangzz_core::message::Message;
use yangzz_core::provider::Provider;
use yangzz_core::render::Renderer;
use yangzz_core::skill::{self, Skill};
use yangzz_core::tool::ToolExecutor;

pub(crate) enum CommandResult {
    Continue,
    Quit,
    FallThrough,
    Unknown,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_command(
    cmd: &str,
    arg: &str,
    current_model: &mut String,
    current_provider: &mut Arc<dyn Provider>,
    messages: &mut Vec<Message>,
    stats: &mut crate::ui::status::SessionStats,
    executor: &ToolExecutor,
    skills: &[Skill],
    settings: &Settings,
) -> CommandResult {
    let registry = slash::build_default();
    let mut ctx = SlashCtx {
        current_model,
        current_provider,
        messages,
        stats,
        settings,
        executor,
        skills,
    };
    let line = if arg.is_empty() {
        cmd.to_string()
    } else {
        format!("{cmd} {arg}")
    };
    match registry.dispatch(&mut ctx, &line) {
        SlashOutcome::Continue => CommandResult::Continue,
        SlashOutcome::Quit => CommandResult::Quit,
        SlashOutcome::Unhandled => {
            if skill::match_skill(cmd, skills).is_some() {
                CommandResult::FallThrough
            } else {
                CommandResult::Unknown
            }
        }
    }
}

pub(crate) fn switch_model_provider(
    new_model: &str,
    provider_name: Option<&str>,
    current_model: &mut String,
    current_provider: &mut Arc<dyn Provider>,
    stats: &mut crate::ui::status::SessionStats,
    renderer: &mut dyn Renderer,
) {
    let mut settings = Settings::load(CliOverrides::default());
    settings.model = Some(new_model.to_string());

    settings.provider = match provider_name {
        Some(provider_name)
            if settings
                .providers
                .iter()
                .any(|provider| provider.name.eq_ignore_ascii_case(provider_name)) =>
        {
            Some(provider_name.to_string())
        }
        Some(provider_name) if config::detect_provider_family(provider_name).is_some() => {
            config::select_provider_name_for_model(
                &settings,
                new_model,
                Some(current_provider.name()),
            )
            .or_else(|| Some(provider_name.to_string()))
        }
        Some(provider_name) => Some(provider_name.to_string()),
        None => config::select_provider_name_for_model(
            &settings,
            new_model,
            Some(current_provider.name()),
        ),
    };

    match config::resolve_provider(&settings) {
        Ok(new_provider) => {
            let old_provider = current_provider.name().to_string();
            *current_model = new_model.to_string();
            *current_provider = new_provider;
            stats.model = current_model.clone();
            stats.provider = current_provider.name().to_string();

            if old_provider != current_provider.name() {
                println!(
                    "  {GREEN}●{RESET} {BOLD}{current_model}{RESET} {DIM}via{RESET} {GOLD}{}{RESET}",
                    current_provider.name()
                );
            } else {
                println!("  {GREEN}●{RESET} {BOLD}{current_model}{RESET}");
            }
        }
        Err(err) => {
            renderer.render_error(&format!("Cannot switch: {err}"));
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn slash_registry_finds_quit_aliases() {
        let registry = crate::slash::build_default();
        assert_eq!(registry.find("quit").unwrap().name(), "quit");
        assert_eq!(registry.find("q").unwrap().name(), "quit");
        assert_eq!(registry.find("exit").unwrap().name(), "quit");
    }

    #[test]
    fn slash_registry_routes_repl_commands() {
        let registry = crate::slash::build_default();

        assert_eq!(registry.find("model").unwrap().name(), "model");
        assert_eq!(registry.find("m").unwrap().name(), "model");
        assert_eq!(registry.find("clear").unwrap().name(), "clear");
        assert_eq!(registry.find("c").unwrap().name(), "clear");
        assert_eq!(registry.find("status").unwrap().name(), "status");
        assert_eq!(registry.find("undo").unwrap().name(), "undo");
        assert_eq!(registry.find("u").unwrap().name(), "undo");
        assert_eq!(registry.find("compact").unwrap().name(), "compact");
        assert_eq!(registry.find("memory").unwrap().name(), "memory");
        assert_eq!(registry.find("recall").unwrap().name(), "recall");
        assert_eq!(registry.find("search").unwrap().name(), "recall");
        assert_eq!(registry.find("task").unwrap().name(), "task");
        assert_eq!(registry.find("tasks").unwrap().name(), "task");
        assert_eq!(registry.find("route").unwrap().name(), "route");
        assert_eq!(registry.find("strategy").unwrap().name(), "strategy");
        assert_eq!(registry.find("profile").unwrap().name(), "profile");
        assert_eq!(registry.find("policy").unwrap().name(), "policy");
        assert_eq!(registry.find("help").unwrap().name(), "help");
        assert_eq!(registry.find("h").unwrap().name(), "help");
        assert_eq!(registry.find("?").unwrap().name(), "help");
        assert_eq!(registry.find("guide").unwrap().name(), "guide");
        assert_eq!(registry.find("migrate").unwrap().name(), "migrate");
        assert_eq!(registry.find("tool").unwrap().name(), "tool");
        assert_eq!(registry.find("tools").unwrap().name(), "tool");
        assert_eq!(registry.find("t").unwrap().name(), "tool");
        assert_eq!(registry.find("skill").unwrap().name(), "skill");
        assert_eq!(registry.find("skills").unwrap().name(), "skills");
        assert_eq!(registry.find("s").unwrap().name(), "skills");
    }
}
