//! /mcp — manage MCP (Model Context Protocol) servers.
//!
//! Global MCP configs live at ~/.yangzz/mcp.json.
//! Project-local configs at ./.yangzz/mcp.json still work and override globals
//! on name collision.

use crate::emitln;

use crate::slash::wizard::Wizard;
use crate::slash::{Category, CommandContext, Outcome, SlashCommand};
use crate::ui::format::*;
use std::collections::HashMap;
use yangzz_core::mcp::{self, McpServerConfig};

pub struct McpCommand;

impl SlashCommand for McpCommand {
    fn name(&self) -> &'static str {
        "mcp"
    }
    fn category(&self) -> Category {
        Category::Extension
    }
    fn summary(&self) -> &'static str {
        "管理 MCP servers"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /mcp                  显示当前 MCP servers\n\
         \x20 /mcp list             列出全部（全局 + 项目）\n\
         \x20 /mcp add              交互式添加到全局 ~/.yangzz/mcp.json\n\
         \x20 /mcp remove <name>    删\n\
         \x20 /mcp status           运行状态（TODO：需要运行时连通性检测）\n\n\
         MCP 是给 AI 助手加外部工具的标准协议（如文件管理、Figma、数据库等）。"
    }

    fn handle(&self, _ctx: &mut CommandContext, args: &str) -> Outcome {
        let mut parts = args.splitn(2, char::is_whitespace);
        let sub = parts.next().unwrap_or("").trim();
        let rest = parts.next().unwrap_or("").trim();

        match sub {
            "" | "list" | "ls" => list_servers(),
            "add" => add_server(),
            "remove" | "rm" | "delete" => {
                if rest.is_empty() {
                    emitln!("  {RED}✖{RESET} 用法：/mcp remove <name>");
                } else {
                    remove_server(rest);
                }
            }
            "status" => status(),
            _ => emitln!("  {RED}✖{RESET} 未知子命令。/mcp 查看用法。"),
        }
        Outcome::Continue
    }
}

fn list_servers() {
    let cwd = std::env::current_dir().unwrap_or_default();
    let global_path = mcp::global_mcp_path();
    let project_path = cwd.join(".yangzz").join("mcp.json");

    let global = mcp::load_mcp_configs_at(&global_path);
    let project = mcp::load_mcp_configs_at(&project_path);

    emitln!();
    emitln!("  {BOLD}MCP Servers{RESET}");
    emitln!();

    if !global.is_empty() {
        emitln!(
            "  {BOLD_GOLD}全局{RESET} {DIM}{}{RESET}",
            global_path.display()
        );
        for s in &global {
            emitln!(
                "    {BOLD}{}{RESET}  {DIM}{} {}{RESET}",
                s.name,
                s.command,
                s.args.join(" ")
            );
        }
    } else {
        emitln!("  {BOLD_GOLD}全局{RESET} {DIM}(空 — 试试 /mcp add){RESET}");
    }

    if !project.is_empty() {
        emitln!();
        emitln!(
            "  {BOLD_GOLD}项目{RESET} {DIM}{}{RESET}",
            project_path.display()
        );
        for s in &project {
            emitln!(
                "    {BOLD}{}{RESET}  {DIM}{} {}{RESET}",
                s.name,
                s.command,
                s.args.join(" ")
            );
        }
    }

    emitln!();
}

fn add_server() {
    let Some(answers) = Wizard::new("添加 MCP server（写入全局 ~/.yangzz/mcp.json）")
        .ask("server 名字（唯一）", None)
        .ask("启动命令（可执行文件路径）", None)
        .ask("命令参数（空格分隔，无则留空）", Some(""))
        .run()
    else {
        return;
    };

    let name = answers[0].trim().to_string();
    let command = answers[1].trim().to_string();
    let args_str = answers[2].trim();

    if name.is_empty() || command.is_empty() {
        emitln!("  {RED}✖{RESET} 名字和命令都不能为空");
        return;
    }

    let args = if args_str.is_empty() {
        Vec::new()
    } else {
        args_str.split_whitespace().map(String::from).collect()
    };

    let mut existing = mcp::load_mcp_configs_at(&mcp::global_mcp_path());
    if existing.iter().any(|c| c.name.eq_ignore_ascii_case(&name)) {
        emitln!("  {RED}✖{RESET} MCP server '{name}' 已存在（全局），先删再加");
        return;
    }

    existing.push(McpServerConfig {
        name: name.clone(),
        command,
        args,
        env: HashMap::new(),
    });

    match mcp::save_global_mcp_configs(&existing) {
        Ok(path) => {
            emitln!();
            emitln!("  {GREEN}✓{RESET} 已添加 {BOLD}{name}{RESET}");
            emitln!("  {DIM}写入: {}{RESET}", path.display());
            emitln!("  {DIM}重启 yangzz 即可加载新 server{RESET}");
            emitln!();
        }
        Err(e) => emitln!("  {RED}✖{RESET} 写入失败: {e}"),
    }
}

fn remove_server(name: &str) {
    let mut configs = mcp::load_mcp_configs_at(&mcp::global_mcp_path());
    let before = configs.len();
    configs.retain(|c| !c.name.eq_ignore_ascii_case(name));
    if configs.len() == before {
        emitln!(
            "  {RED}✖{RESET} 全局 mcp.json 里没有 '{name}'（如果是项目级，请手动改 .yangzz/mcp.json）"
        );
        return;
    }
    match mcp::save_global_mcp_configs(&configs) {
        Ok(_) => emitln!("  {GREEN}✓{RESET} 已删除 {BOLD}{name}{RESET}"),
        Err(e) => emitln!("  {RED}✖{RESET} 保存失败: {e}"),
    }
}

fn status() {
    // TODO: wire this to actual McpManager runtime state once we thread it
    // through ReplContext. For now show config info.
    let cwd = std::env::current_dir().unwrap_or_default();
    let total = mcp::load_mcp_configs(&cwd).len();
    emitln!(
        "  {DIM}已配置{RESET} {BOLD}{}{RESET} {DIM}个 MCP server{RESET}",
        total
    );
    emitln!("  {DIM}（详细运行状态将在 v0.3.1 提供）{RESET}");
}
