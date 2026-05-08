use anyhow::Result;
use std::sync::Arc;

use crate::slash::{self, CommandContext, Outcome};
use crate::ui::status::SessionStats;
use yangzz_core::config::settings::CliOverrides;
use yangzz_core::config::{self, Settings};
use yangzz_core::message::Message;
use yangzz_core::provider::{OpenAiCompatProvider, Provider};
use yangzz_core::skill;
use yangzz_core::tool::ToolExecutor;

#[derive(Debug, Clone)]
pub struct DetectedCliCommand {
    pub parse_args: Vec<String>,
    pub command_name: String,
    pub command_args: Vec<String>,
}

/// Detect `yangzz provider add`-style invocations before clap parses them as a
/// free-form prompt. Only global flags before the command are recognized here.
pub fn detect_cli_command(raw_args: &[String]) -> Option<DetectedCliCommand> {
    if raw_args.len() <= 1 {
        return None;
    }

    let registry = slash::build_default();
    let mut parse_args = vec![raw_args[0].clone()];
    let mut idx = 1usize;

    while idx < raw_args.len() {
        let arg = &raw_args[idx];
        if registry.find(arg).is_some() {
            return Some(DetectedCliCommand {
                parse_args,
                command_name: arg.clone(),
                command_args: raw_args[idx + 1..].to_vec(),
            });
        }

        if !arg.starts_with('-') {
            return None;
        }

        parse_args.push(arg.clone());
        if takes_value(arg) {
            idx += 1;
            if idx >= raw_args.len() {
                break;
            }
            parse_args.push(raw_args[idx].clone());
        }
        idx += 1;
    }

    None
}

fn takes_value(arg: &str) -> bool {
    matches!(
        arg,
        "--provider" | "--model" | "-m" | "--api-key" | "--api_key" | "--base-url" | "--base_url"
    )
}

pub async fn run_cli_command(
    detected: &DetectedCliCommand,
    cli_overrides: CliOverrides,
    executor: &ToolExecutor,
) -> Result<()> {
    let settings = Settings::load(cli_overrides);
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut skills = skill::builtin_skills();
    skills.extend(skill::load_skills(&cwd));

    let mut current_model = settings.resolved_model();
    let mut current_provider: Arc<dyn Provider> = match config::resolve_provider(&settings) {
        Ok(provider) => provider,
        Err(_) => Arc::new(OpenAiCompatProvider::new(
            settings.provider.as_deref().unwrap_or("unconfigured"),
            "",
            "http://localhost:11434",
            Some(current_model.clone()),
        )?),
    };
    let mut messages: Vec<Message> = Vec::new();
    let mut stats = SessionStats::new(&current_model, current_provider.name());
    let registry = slash::build_default();

    let arg_string = detected.command_args.join(" ");
    let line = if arg_string.is_empty() {
        format!("/{}", detected.command_name)
    } else if detected
        .command_args
        .iter()
        .any(|arg| arg == "--help" || arg == "-h")
    {
        format!("/help {}", detected.command_name)
    } else {
        format!("/{} {}", detected.command_name, arg_string)
    };

    let mut ctx = CommandContext {
        current_model: &mut current_model,
        current_provider: &mut current_provider,
        messages: &mut messages,
        stats: &mut stats,
        settings: &settings,
        executor,
        skills: &skills,
    };

    match registry.dispatch(&mut ctx, &line) {
        Outcome::Continue | Outcome::Quit => Ok(()),
        Outcome::Unhandled => anyhow::bail!("Unknown command: {}", detected.command_name),
    }
}
