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
        "查看 / 轮换 API Key"
    }
    fn detailed_help(&self) -> &'static str {
        "用法:\n\
         \x20 /key\n\
         \x20 /key list\n\
         \x20 /key set <name>\n\
         \x20 /key set <name> <newkey>\n\
         \x20 /key <name>\n\
         \x20 /key <name> <newkey>\n\n\
         兼容旧写法：/key <name> 仍然可用。"
    }

    fn handle(&self, _ctx: &mut CommandContext, args: &str) -> Outcome {
        match parse_key_action(args) {
            KeyAction::List => show_all_keys(),
            KeyAction::Set {
                provider,
                inline_key,
            } => {
                let new_key = if let Some(new_key) = inline_key.filter(|value| !value.is_empty()) {
                    new_key.to_string()
                } else {
                    let Some(answers) = Wizard::new(&format!("为 {provider} 设置新 Key"))
                        .ask_secret("新 API Key")
                        .run()
                    else {
                        return Outcome::Continue;
                    };
                    answers[0].clone()
                };

                match writer::rotate_api_key(provider, &new_key) {
                    Ok(_) => emitln!("  {GREEN}✓{RESET} 已更新 {BOLD}{provider}{RESET} 的 API Key"),
                    Err(e) => emitln!("  {RED}✖{RESET} {e}"),
                }
            }
            KeyAction::InvalidUsage => {
                emitln!("  {RED}✖{RESET} 用法：/key [list|set <provider> [newkey]]");
            }
        }
        Outcome::Continue
    }
}

#[derive(Debug, PartialEq, Eq)]
enum KeyAction<'a> {
    List,
    Set {
        provider: &'a str,
        inline_key: Option<&'a str>,
    },
    InvalidUsage,
}

fn parse_key_action(args: &str) -> KeyAction<'_> {
    let args = args.trim();
    if args.is_empty() || matches!(args, "list" | "ls") {
        return KeyAction::List;
    }

    let mut parts = args.splitn(3, char::is_whitespace);
    let first = parts.next().unwrap_or("").trim();

    if first.eq_ignore_ascii_case("set") {
        let provider = parts.next().unwrap_or("").trim();
        if provider.is_empty() {
            return KeyAction::InvalidUsage;
        }
        let inline_key = parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        return KeyAction::Set {
            provider,
            inline_key,
        };
    }

    let inline_key = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if first.is_empty() {
        KeyAction::InvalidUsage
    } else {
        KeyAction::Set {
            provider: first,
            inline_key,
        }
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
    emitln!("  {DIM}查看：/key list{RESET}");
    emitln!("  {DIM}轮换：/key set <name> [newkey]{RESET}");
    emitln!("  {DIM}兼容旧写法：/key <name> [newkey]{RESET}");
    emitln!();
}

fn mask(k: &str) -> String {
    if k.len() <= 8 {
        return "(hidden)".into();
    }
    let tail = &k[k.len().saturating_sub(4)..];
    format!("****{tail}")
}

#[cfg(test)]
mod tests {
    use super::{KeyAction, parse_key_action};

    #[test]
    fn parse_key_list_variants() {
        assert_eq!(parse_key_action(""), KeyAction::List);
        assert_eq!(parse_key_action("list"), KeyAction::List);
        assert_eq!(parse_key_action("ls"), KeyAction::List);
    }

    #[test]
    fn parse_key_set_variants() {
        assert_eq!(
            parse_key_action("set relay sk-123"),
            KeyAction::Set {
                provider: "relay",
                inline_key: Some("sk-123"),
            }
        );
        assert_eq!(
            parse_key_action("relay sk-456"),
            KeyAction::Set {
                provider: "relay",
                inline_key: Some("sk-456"),
            }
        );
        assert_eq!(
            parse_key_action("relay"),
            KeyAction::Set {
                provider: "relay",
                inline_key: None,
            }
        );
    }

    #[test]
    fn parse_key_requires_provider_after_set() {
        assert_eq!(parse_key_action("set"), KeyAction::InvalidUsage);
        assert_eq!(parse_key_action("set   "), KeyAction::InvalidUsage);
    }
}
