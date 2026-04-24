//! /skill — list / add (copy file) / reload / show
//!
//! Skills are `.md` files under `.yangzz/skills/` (project) or
//! `~/.yangzz/skills/` (global — introduced in v0.3.0).

use crate::emitln;

use crate::slash::{Category, CommandContext, Outcome, SlashCommand};
use crate::ui::format::*;
use crate::ui::i18n::translate_skill_desc;
use std::path::{Path, PathBuf};

pub struct SkillCommand;
pub struct SkillsCommand;

impl SlashCommand for SkillCommand {
    fn name(&self) -> &'static str {
        "skill"
    }
    fn category(&self) -> Category {
        Category::Extension
    }
    fn summary(&self) -> &'static str {
        "查看 / 添加 / 重载 skill"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /skill                显示当前加载的 skills\n\
         \x20 /skill list           列表\n\
         \x20 /skill add <path>     复制外部 .md 到 ~/.yangzz/skills/\n\
         \x20 /skill show <name>    显示某个 skill 的内容\n\
         \x20 /skill reload         重扫 skills 目录（需重启 REPL 生效）\n\n\
         Skill 是可复用的提示词模板（.md 文件，带 yaml 前言声明触发词）。"
    }

    fn handle(&self, _ctx: &mut CommandContext, args: &str) -> Outcome {
        let mut parts = args.splitn(2, char::is_whitespace);
        let sub = parts.next().unwrap_or("").trim();
        let rest = parts.next().unwrap_or("").trim();

        match sub {
            "" | "list" | "ls" => list_skills(),
            "add" => {
                if rest.is_empty() {
                    emitln!("  {RED}✖{RESET} 用法：/skill add <file.md>");
                } else {
                    add_skill(rest);
                }
            }
            "show" => {
                if rest.is_empty() {
                    emitln!("  {RED}✖{RESET} 用法：/skill show <name>");
                } else {
                    show_skill(rest);
                }
            }
            "reload" => emitln!("  {DIM}skills 目前在启动时加载，下次启动会重扫。{RESET}"),
            _ => emitln!("  {RED}✖{RESET} 未知子命令。/skill 查看用法。"),
        }
        Outcome::Continue
    }
}

impl SlashCommand for SkillsCommand {
    fn name(&self) -> &'static str {
        "skills"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["s"]
    }
    fn category(&self) -> Category {
        Category::Extension
    }
    fn summary(&self) -> &'static str {
        "查看当前已加载 skills"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /skills          显示当前已加载的 skills\n\
         \x20 /s               同上\n\
         \x20 /skills list     同上"
    }

    fn handle(&self, ctx: &mut CommandContext, args: &str) -> Outcome {
        let sub = args.trim();
        if !sub.is_empty() && !matches!(sub, "list" | "ls") {
            emitln!("  {RED}✖{RESET} 未知子命令。/skills 查看用法。");
            return Outcome::Continue;
        }

        emitln!();
        emitln!("  {BOLD}Skills{RESET}");
        for sk in ctx.skills {
            let trigger = sk
                .triggers
                .iter()
                .find(|t| t.starts_with('/'))
                .cloned()
                .unwrap_or_default();
            let desc = translate_skill_desc(&sk.name, &sk.description);
            emitln!("    {GOLD}{:<14}{RESET} {DIM}{desc}{RESET}", trigger);
        }
        emitln!();
        Outcome::Continue
    }
}

fn global_skills_dir() -> PathBuf {
    yangzz_core::paths::yangzz_dir().join("skills")
}

fn list_skills() {
    let cwd = std::env::current_dir().unwrap_or_default();
    let global_dir = global_skills_dir();
    let project_dir = cwd.join(".yangzz").join("skills");

    let global = list_md_files(&global_dir);
    let project = list_md_files(&project_dir);

    emitln!();
    emitln!("  {BOLD}Skills{RESET}");
    emitln!();

    if !global.is_empty() {
        emitln!(
            "  {BOLD_GOLD}全局{RESET} {DIM}{}{RESET}",
            global_dir.display()
        );
        for f in &global {
            emitln!("    {BOLD}{}{RESET}", f);
        }
    } else {
        emitln!("  {BOLD_GOLD}全局{RESET} {DIM}(空 — /skill add <file.md> 添加){RESET}");
    }

    if !project.is_empty() {
        emitln!();
        emitln!(
            "  {BOLD_GOLD}项目{RESET} {DIM}{}{RESET}",
            project_dir.display()
        );
        for f in &project {
            emitln!("    {BOLD}{}{RESET}", f);
        }
    }
    emitln!();
}

fn list_md_files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "md") {
                if let Some(name) = p.file_stem().and_then(|s| s.to_str()) {
                    out.push(name.to_string());
                }
            }
        }
    }
    out.sort();
    out
}

fn add_skill(src: &str) {
    let src_path = Path::new(src);
    if !src_path.exists() {
        emitln!("  {RED}✖{RESET} 文件不存在: {src}");
        return;
    }
    if src_path.extension().is_none_or(|x| x != "md") {
        emitln!("  {RED}✖{RESET} skill 必须是 .md 文件");
        return;
    }
    let dir = global_skills_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        emitln!("  {RED}✖{RESET} 创建目录失败: {e}");
        return;
    }
    let file_name = src_path.file_name().unwrap_or_default();
    let dst = dir.join(file_name);
    if dst.exists() {
        emitln!("  {RED}✖{RESET} {} 已存在，先删或改名", dst.display());
        return;
    }
    match std::fs::copy(src_path, &dst) {
        Ok(_) => {
            emitln!("  {GREEN}✓{RESET} 已添加到 {}", dst.display());
            emitln!("  {DIM}重启 yangzz 后生效{RESET}");
        }
        Err(e) => emitln!("  {RED}✖{RESET} 复制失败: {e}"),
    }
}

fn show_skill(name: &str) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let candidates = [
        global_skills_dir().join(format!("{name}.md")),
        cwd.join(".yangzz")
            .join("skills")
            .join(format!("{name}.md")),
    ];
    for path in &candidates {
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    emitln!();
                    emitln!("  {DIM}{}{RESET}", path.display());
                    emitln!("  {DIM}───────────{RESET}");
                    for line in content.lines().take(60) {
                        emitln!("  {line}");
                    }
                    if content.lines().count() > 60 {
                        emitln!("  {DIM}... (截断，更多内容请直接打开文件){RESET}");
                    }
                    emitln!();
                }
                Err(e) => emitln!("  {RED}✖{RESET} 读取失败: {e}"),
            }
            return;
        }
    }
    emitln!("  {RED}✖{RESET} 未找到 skill '{name}'");
}
