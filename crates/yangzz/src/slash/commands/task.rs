use std::sync::OnceLock;

use crate::emitln;
use crate::slash::{Category, CommandContext, Outcome, SlashCommand, block_on};
use crate::ui::format::*;
use yangzz_core::task_queue::{TaskPriority, TaskQueue, TaskType};

pub struct TaskCommand;
pub struct RouteCommand;
pub struct StrategyCommand;
pub struct ProfileCommand;
pub struct PolicyCommand;

impl SlashCommand for TaskCommand {
    fn name(&self) -> &'static str {
        "task"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["tasks"]
    }
    fn category(&self) -> Category {
        Category::Task
    }
    fn summary(&self) -> &'static str {
        "任务队列：list/add/done/cancel"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /task\n\
         \x20 /tasks\n\
         \x20 /task list\n\
         \x20 /task add <description>\n\
         \x20 /task done <id>\n\
         \x20 /task cancel <id>"
    }

    fn handle(&self, _ctx: &mut CommandContext, args: &str) -> Outcome {
        let arg = args.trim();
        let queue = task_queue();

        if arg.is_empty() || matches!(arg, "list" | "ls") {
            let list = block_on(queue.format_list());
            emitln!("  {BOLD}Task Queue:{RESET}");
            emitln!("  {list}");
            return Outcome::Continue;
        }

        if let Some(desc) = arg.strip_prefix("add ").map(str::trim) {
            if desc.is_empty() {
                emitln!("  {RED}✖{RESET} 用法：/task add <description>");
                return Outcome::Continue;
            }
            let id = block_on(queue.enqueue(TaskType::Agent, desc, TaskPriority::Normal));
            emitln!("  {GREEN}✓{RESET} Task #{id} queued: {desc}");
            return Outcome::Continue;
        }

        if let Some(id) = parse_task_id(arg, "done") {
            block_on(queue.complete(id, "Completed via /task done".into()));
            emitln!("  {GREEN}✓{RESET} Task #{id} marked complete");
            return Outcome::Continue;
        }

        if let Some(id) = parse_task_id(arg, "cancel") {
            if block_on(queue.cancel(id)) {
                emitln!("  {GREEN}✓{RESET} Task #{id} cancelled");
            } else {
                emitln!("  {RED}✖{RESET} Cannot cancel task #{id}");
            }
            return Outcome::Continue;
        }

        emitln!("  {RED}✖{RESET} 用法：/task [list|add <desc>|done <id>|cancel <id>]");
        Outcome::Continue
    }
}

impl SlashCommand for RouteCommand {
    fn name(&self) -> &'static str {
        "route"
    }
    fn category(&self) -> Category {
        Category::Task
    }
    fn summary(&self) -> &'static str {
        "预览 prompt 的自动路由结果"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /route <prompt>"
    }

    fn handle(&self, ctx: &mut CommandContext, args: &str) -> Outcome {
        let prompt = args.trim();
        if prompt.is_empty() {
            emitln!("  {RED}✖{RESET} 用法：/route <prompt>");
            return Outcome::Continue;
        }

        if let Some(strategy) = &ctx.settings.strategy {
            let router = yangzz_core::provider::router::StrategyRouter::new(strategy);
            let decision = router.route(prompt);
            emitln!("  {BOLD}Strategy Route:{RESET}");
            emitln!("    {GOLD}Domain:{RESET}   {}", decision.domain.as_str());
            emitln!("    {GOLD}Provider:{RESET} {}", decision.provider_name);
            emitln!("    {DIM}{}{RESET}", decision.reason);

            let tasks = router.decompose_task(prompt);
            if tasks.len() > 1 {
                emitln!();
                emitln!("  {BOLD}Multi-domain decomposition:{RESET}");
                for (domain, _) in &tasks {
                    let provider_name = router.provider_for_domain(*domain).unwrap_or_default();
                    emitln!("    {GOLD}{}{RESET} → {}", domain.as_str(), provider_name);
                }
            }
        } else {
            let router = yangzz_core::provider::router::ModelRouter::new();
            let decision = router.route(prompt, &["openai", "anthropic", "deepseek", "gemini"]);
            emitln!("  {BOLD}Complexity:{RESET} {:?}", decision.complexity);
            emitln!("  {BOLD}Model:{RESET} {}", decision.model);
            emitln!("  {DIM}{}{RESET}", decision.reason);
            emitln!();
            emitln!("  {DIM}Tip: 配置 [strategy] 启用多模型自动路由{RESET}");
        }

        Outcome::Continue
    }
}

impl SlashCommand for StrategyCommand {
    fn name(&self) -> &'static str {
        "strategy"
    }
    fn category(&self) -> Category {
        Category::Task
    }
    fn summary(&self) -> &'static str {
        "查看多模型 / 多 agent 路由配置"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /strategy"
    }

    fn handle(&self, ctx: &mut CommandContext, _args: &str) -> Outcome {
        if let Some(strategy) = &ctx.settings.strategy {
            emitln!("  {BOLD}Strategy Config:{RESET}");
            emitln!("    {GOLD}Mode:{RESET} {}", strategy.mode);
            emitln!();
            emitln!("  {BOLD}Role → Provider:{RESET}");
            emit_strategy_role("planner", &strategy.roles.planner);
            emit_strategy_role("frontend", &strategy.roles.frontend);
            emit_strategy_role("backend", &strategy.roles.backend);
            emit_strategy_role("review", &strategy.roles.review);
            emit_strategy_role("test", &strategy.roles.test);
            emit_strategy_role("general", &strategy.roles.general);
        } else {
            emitln!("  {DIM}No [strategy] section in config.{RESET}");
            emitln!("  Add to config.toml:");
            emitln!();
            emitln!("    {BOLD}[strategy]{RESET}");
            emitln!("    {BOLD}mode = \"auto\"{RESET}");
            emitln!();
            emitln!("    {BOLD}[strategy.roles]{RESET}");
            emitln!("    {BOLD}planner = \"claude-relay\"{RESET}");
            emitln!("    {BOLD}frontend = \"claude-relay\"{RESET}");
            emitln!("    {BOLD}backend = \"openai-relay\"{RESET}");
            emitln!("    {BOLD}review = \"gemini-relay\"{RESET}");
            emitln!("    {BOLD}test = \"deepseek-relay\"{RESET}");
            emitln!("    {BOLD}general = \"openai-relay\"{RESET}");
        }
        Outcome::Continue
    }
}

impl SlashCommand for ProfileCommand {
    fn name(&self) -> &'static str {
        "profile"
    }
    fn category(&self) -> Category {
        Category::Task
    }
    fn summary(&self) -> &'static str {
        "自动识别当前项目技术栈"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /profile"
    }

    fn handle(&self, _ctx: &mut CommandContext, _args: &str) -> Outcome {
        let cwd = std::env::current_dir().unwrap_or_default();
        let profile = yangzz_core::skill_detect::detect_project(&cwd);

        emitln!("  {BOLD}Project Profile:{RESET}");
        if !profile.languages.is_empty() {
            emitln!("  {GOLD}Languages:{RESET} {}", profile.languages.join(", "));
        }
        if !profile.frameworks.is_empty() {
            emitln!(
                "  {GOLD}Frameworks:{RESET} {}",
                profile.frameworks.join(", ")
            );
        }
        if !profile.package_managers.is_empty() {
            emitln!(
                "  {GOLD}Package Managers:{RESET} {}",
                profile.package_managers.join(", ")
            );
        }
        if let Some(project_type) = &profile.project_type {
            emitln!("  {GOLD}Type:{RESET} {project_type}");
        }
        emitln!(
            "  {DIM}Tests: {} | CI: {} | Docker: {} | Git: {}{RESET}",
            if profile.has_tests { "✅" } else { "❌" },
            if profile.has_ci { "✅" } else { "❌" },
            if profile.has_docker { "✅" } else { "❌" },
            if profile.has_git { "✅" } else { "❌" },
        );
        Outcome::Continue
    }
}

impl SlashCommand for PolicyCommand {
    fn name(&self) -> &'static str {
        "policy"
    }
    fn category(&self) -> Category {
        Category::Task
    }
    fn summary(&self) -> &'static str {
        "查看当前执行沙箱策略"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /policy"
    }

    fn handle(&self, _ctx: &mut CommandContext, _args: &str) -> Outcome {
        let cwd = std::env::current_dir().unwrap_or_default();
        let policy = yangzz_core::sandbox::load_policy(&cwd);

        emitln!("  {BOLD}Execution Policy:{RESET}");
        emitln!("  Sandbox: {}", policy.sandbox.mode);
        emitln!(
            "  Network outbound: {}",
            if policy.network.allow_outbound {
                "✅"
            } else {
                "❌"
            }
        );
        if !policy.network.blocked_hosts.is_empty() {
            emitln!(
                "  Blocked hosts: {}",
                policy.network.blocked_hosts.join(", ")
            );
        }
        if !policy.commands.blocked_commands.is_empty() {
            emitln!(
                "  Blocked commands: {}",
                policy.commands.blocked_commands.join(", ")
            );
        }
        emitln!("  Max runtime: {}s", policy.commands.max_runtime_secs);
        emitln!(
            "  Sudo: {}",
            if policy.commands.allow_sudo {
                "✅"
            } else {
                "❌"
            }
        );
        Outcome::Continue
    }
}

fn task_queue() -> &'static TaskQueue {
    static TASK_QUEUE: OnceLock<TaskQueue> = OnceLock::new();
    TASK_QUEUE.get_or_init(TaskQueue::new)
}

fn parse_task_id(args: &str, subcommand: &str) -> Option<usize> {
    args.strip_prefix(&format!("{subcommand} "))
        .map(str::trim)
        .and_then(|value| value.parse::<usize>().ok())
}

fn emit_strategy_role(name: &str, provider_name: &Option<String>) {
    if let Some(provider_name) = provider_name {
        emitln!(
            "    {GOLD}{:<12}{RESET} → {BOLD}{}{RESET}",
            name,
            provider_name
        );
    } else {
        emitln!("    {DIM}{:<12} → (not set){RESET}", name);
    }
}

#[cfg(test)]
mod tests {
    use super::{PolicyCommand, ProfileCommand, RouteCommand, StrategyCommand, TaskCommand};
    use crate::slash::{Category, SlashCommand};

    #[test]
    fn task_commands_use_task_category() {
        assert_eq!(TaskCommand.category(), Category::Task);
        assert_eq!(RouteCommand.category(), Category::Task);
        assert_eq!(StrategyCommand.category(), Category::Task);
        assert_eq!(ProfileCommand.category(), Category::Task);
        assert_eq!(PolicyCommand.category(), Category::Task);
    }

    #[test]
    fn task_command_alias_matches_tasks() {
        assert_eq!(TaskCommand.aliases(), &["tasks"]);
    }
}
