use crate::emitln;
use crate::slash;
use crate::ui::format::*;
use crate::ui::i18n::{Strings, t, translate_skill_desc};
use yangzz_core::skill::Skill;

#[derive(Clone, Copy)]
struct HelpEntry<'a> {
    picker_name: Option<&'a str>,
    command: &'a str,
    alias: Option<&'a str>,
    usage: Option<&'a str>,
    desc: &'a str,
}

pub(crate) fn all_commands(skills: &[Skill]) -> Vec<(String, String)> {
    let mut cmds = Vec::new();
    for entry in help_entries(t()) {
        if let Some(name) = entry.picker_name {
            push_command(&mut cmds, name, entry.desc);
        }
    }

    for skill in skills {
        if let Some(trigger) = skill.triggers.iter().find(|t| t.starts_with('/')) {
            push_command(&mut cmds, trigger, &skill.description);
        }
    }
    cmds
}

fn help_entries<'a>(s: &'a Strings) -> Vec<HelpEntry<'a>> {
    vec![
        HelpEntry {
            picker_name: Some("/help"),
            command: "/help",
            alias: Some("/h"),
            usage: None,
            desc: s.help_help,
        },
        HelpEntry {
            picker_name: Some("/model"),
            command: "/model",
            alias: Some("/m"),
            usage: None,
            desc: s.help_model,
        },
        HelpEntry {
            picker_name: None,
            command: "/model",
            alias: None,
            usage: Some("<name>"),
            desc: s.help_model_name,
        },
        HelpEntry {
            picker_name: Some("/provider"),
            command: "/provider",
            alias: Some("/p"),
            usage: Some("<name>"),
            desc: s.help_provider,
        },
        HelpEntry {
            picker_name: Some("/key"),
            command: "/key",
            alias: None,
            usage: None,
            desc: "Rotate API keys",
        },
        HelpEntry {
            picker_name: Some("/config"),
            command: "/config",
            alias: None,
            usage: None,
            desc: "Show effective config",
        },
        HelpEntry {
            picker_name: Some("/tools"),
            command: "/tools",
            alias: Some("/t"),
            usage: None,
            desc: s.help_tools,
        },
        HelpEntry {
            picker_name: Some("/mcp"),
            command: "/mcp",
            alias: None,
            usage: None,
            desc: "Manage MCP servers",
        },
        HelpEntry {
            picker_name: Some("/skills"),
            command: "/skills",
            alias: Some("/s"),
            usage: None,
            desc: s.help_skills,
        },
        HelpEntry {
            picker_name: Some("/clear"),
            command: "/clear",
            alias: Some("/c"),
            usage: None,
            desc: s.help_clear,
        },
        HelpEntry {
            picker_name: Some("/status"),
            command: "/status",
            alias: None,
            usage: None,
            desc: s.help_status,
        },
        HelpEntry {
            picker_name: Some("/undo"),
            command: "/undo",
            alias: Some("/u"),
            usage: None,
            desc: "Undo last file edit (max 20)",
        },
        HelpEntry {
            picker_name: Some("/compact"),
            command: "/compact",
            alias: None,
            usage: None,
            desc: "Compact conversation history",
        },
        HelpEntry {
            picker_name: Some("/memory"),
            command: "/memory",
            alias: None,
            usage: Some("[text]"),
            desc: "View/add MEMORY.md entries",
        },
        HelpEntry {
            picker_name: Some("/recall"),
            command: "/recall",
            alias: None,
            usage: Some("<keyword>"),
            desc: "Search across past sessions",
        },
        HelpEntry {
            picker_name: Some("/migrate"),
            command: "/migrate",
            alias: None,
            usage: None,
            desc: "Import config from Claude Code/Codex/Cursor",
        },
        HelpEntry {
            picker_name: Some("/task"),
            command: "/task",
            alias: None,
            usage: Some("[cmd]"),
            desc: "Task queue: list/add/done/cancel",
        },
        HelpEntry {
            picker_name: Some("/route"),
            command: "/route",
            alias: None,
            usage: Some("<prompt>"),
            desc: "Smart model routing preview",
        },
        HelpEntry {
            picker_name: Some("/strategy"),
            command: "/strategy",
            alias: None,
            usage: None,
            desc: "View multi-agent strategy config",
        },
        HelpEntry {
            picker_name: Some("/profile"),
            command: "/profile",
            alias: None,
            usage: None,
            desc: "Auto-detected project profile",
        },
        HelpEntry {
            picker_name: Some("/policy"),
            command: "/policy",
            alias: None,
            usage: None,
            desc: "Show execution policy",
        },
        HelpEntry {
            picker_name: Some("/guide"),
            command: "/guide",
            alias: None,
            usage: None,
            desc: "Quick-start guide",
        },
        HelpEntry {
            picker_name: Some("/quit"),
            command: "/quit",
            alias: Some("/q"),
            usage: None,
            desc: s.help_quit,
        },
    ]
}

fn push_command(
    commands: &mut Vec<(String, String)>,
    name: impl AsRef<str>,
    desc: impl AsRef<str>,
) {
    let name = name.as_ref();
    if commands.iter().any(|(existing, _)| existing == name) {
        return;
    }
    commands.push((name.to_string(), desc.as_ref().to_string()));
}

pub(crate) fn print_help(skills: &[Skill], topic: &str) {
    let s = t();
    let registry = slash::build_default();
    let topic = topic.trim().trim_start_matches('/');

    if !topic.is_empty() {
        if let Some(cmd) = registry.find(topic) {
            emitln!();
            emitln!("  {BOLD}/{}{RESET}", cmd.name());
            emitln!("  {DIM}分类：{}{RESET}", cmd.category().title());
            if !cmd.aliases().is_empty() {
                emitln!("  {DIM}别名：/{0}{RESET}", cmd.aliases().join("  /"));
            }
            emitln!();
            let detail = cmd.detailed_help().trim();
            if detail.is_empty() {
                emitln!("  {}", cmd.summary());
            } else {
                for line in detail.lines() {
                    emitln!("  {line}");
                }
            }
            emitln!();
            emitln!("  {DIM}提示：/help 查看全部命令{RESET}");
            return;
        }
        let legacy_entries = legacy_topic_entries(topic, s);
        if !legacy_entries.is_empty() {
            print_legacy_help_topic(&legacy_entries);
            return;
        }
    }

    emitln!();
    emitln!("  {BOLD}{}{RESET}", s.help_title);
    for entry in help_entries(s) {
        print_help_index_entry(entry);
    }
    emitln!();
    emitln!("  {BOLD}统一子命令（v0.3）{RESET}");
    for (category, commands) in registry.grouped() {
        emitln!("    {SOFT_GOLD}{}{RESET}", category.title());
        for cmd in commands {
            let alias_suffix = if cmd.aliases().is_empty() {
                String::new()
            } else {
                format!(" {DIM}(/{} ){}", cmd.aliases().join(", /"), RESET)
            };
            emitln!(
                "      {GOLD}/{:<10}{RESET}{} {DIM}{}{RESET}",
                cmd.name(),
                alias_suffix,
                cmd.summary()
            );
        }
    }
    if !skills.is_empty() {
        emitln!();
        emitln!("  {BOLD}{}{RESET}", s.skills_title);
        for sk in skills {
            let slash = sk
                .triggers
                .iter()
                .find(|t| t.starts_with('/'))
                .cloned()
                .unwrap_or_default();
            let desc = translate_skill_desc(&sk.name, &sk.description);
            emitln!("    {GOLD}{slash:<12}{RESET} {DIM}{desc}{RESET}");
        }
    }
    emitln!();
    emitln!("  {DIM}{}{RESET}", s.env_hint);
    emitln!();
}

fn legacy_topic_entries<'a>(topic: &str, s: &'a Strings) -> Vec<HelpEntry<'a>> {
    let topic = topic.trim().trim_start_matches('/');
    if topic.is_empty() {
        return Vec::new();
    }

    let entries = help_entries(s);
    let Some(command) = entries
        .iter()
        .find(|entry| help_entry_matches_topic(entry, topic))
        .map(|entry| entry.command)
    else {
        return Vec::new();
    };

    entries
        .into_iter()
        .filter(|entry| entry.command == command)
        .collect()
}

fn print_help_index_entry(entry: HelpEntry<'_>) {
    let command = format!("{GOLD}{}{RESET}", entry.command);
    let alias = entry
        .alias
        .map(|alias| format!(" {GOLD}{alias}{RESET}"))
        .unwrap_or_default();
    let usage = entry
        .usage
        .map(|usage| format!(" {DIM}{usage}{RESET}"))
        .unwrap_or_default();
    emitln!("    {command}{alias}{usage}   {}", entry.desc);
}

fn print_legacy_help_topic(entries: &[HelpEntry<'_>]) {
    if entries.is_empty() {
        return;
    }

    let first = entries[0];
    emitln!();
    emitln!("  {BOLD}{}{RESET}", first.command);
    emitln!("  {DIM}分类：传统 REPL 命令{RESET}");
    if let Some(alias) = first.alias {
        emitln!("  {DIM}别名：{}{RESET}", alias);
    }
    emitln!();
    for entry in entries {
        let usage = entry
            .usage
            .map(|usage| format!(" {DIM}{usage}{RESET}"))
            .unwrap_or_default();
        emitln!("  {GOLD}{}{RESET}{usage}", entry.command);
        emitln!("    {}", entry.desc);
    }
    emitln!();
    emitln!("  {DIM}提示：/help 查看全部命令{RESET}");
}

fn help_entry_matches_topic(entry: &HelpEntry<'_>, topic: &str) -> bool {
    entry
        .command
        .trim_start_matches('/')
        .eq_ignore_ascii_case(topic)
        || entry
            .picker_name
            .is_some_and(|name| name.trim_start_matches('/').eq_ignore_ascii_case(topic))
        || entry
            .alias
            .is_some_and(|alias| alias.trim_start_matches('/').eq_ignore_ascii_case(topic))
}

#[cfg(test)]
mod tests {
    use super::{all_commands, help_entries, legacy_topic_entries};
    use crate::ui::i18n::t;
    use yangzz_core::skill::Skill;

    #[test]
    fn all_commands_include_repl_registry_entries_without_duplicates() {
        let commands = all_commands(&[]);
        let names: Vec<&str> = commands.iter().map(|(name, _)| name.as_str()).collect();

        assert!(names.contains(&"/key"));
        assert!(names.contains(&"/config"));
        assert!(names.contains(&"/mcp"));
        assert_eq!(names.iter().filter(|name| **name == "/provider").count(), 1);
        assert_eq!(names.iter().filter(|name| **name == "/quit").count(), 1);
    }

    #[test]
    fn help_entries_keep_display_only_variants_out_of_picker() {
        let entries = help_entries(t());
        let model_variants = entries
            .iter()
            .filter(|entry| entry.command == "/model")
            .collect::<Vec<_>>();

        assert_eq!(model_variants.len(), 2);
        assert_eq!(
            model_variants
                .iter()
                .filter(|entry| entry.picker_name == Some("/model"))
                .count(),
            1
        );
        assert_eq!(
            model_variants
                .iter()
                .filter(|entry| entry.usage == Some("<name>"))
                .count(),
            1
        );
    }

    #[test]
    fn legacy_topic_entries_match_alias_and_include_command_variants() {
        let entries = legacy_topic_entries("m", t());

        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| entry.usage.is_none()));
        assert!(entries.iter().any(|entry| entry.usage == Some("<name>")));
    }

    #[test]
    fn all_commands_append_skill_slash_trigger() {
        let commands = all_commands(&[Skill {
            name: "ship".into(),
            description: "Ship the current task".into(),
            triggers: vec!["ship it".into(), "/ship".into()],
            body: "do it".into(),
        }]);

        assert!(
            commands
                .iter()
                .any(|(name, desc)| name == "/ship" && desc == "Ship the current task")
        );
    }
}
