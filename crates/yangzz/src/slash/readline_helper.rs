//! rustyline `Helper` that gives:
//! - Slash commands Tab-completion and ghost-hint
//! - @file path Tab-completion (for attaching files)
//!
//! How it feels to the user:
//! - Type `/pr` → ghost hint shows `ovider`. Tab completes.
//! - Type `@src/` → Tab lists files in src/ directory.
//! - Type `@Cargo` → Tab completes to `@Cargo.toml`.

use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};
use std::borrow::Cow;
use std::path::Path;

use super::build_default;

/// The list of canonical slash commands + one-line summaries.
fn all_known_commands() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();

    let registry = build_default();
    for cmd in registry.all() {
        out.push((format!("/{}", cmd.name()), cmd.summary().to_string()));
    }

    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

pub struct YangzzHelper {
    commands: Vec<(String, String)>,
}

impl YangzzHelper {
    pub fn new() -> Self {
        Self {
            commands: all_known_commands(),
        }
    }

    #[allow(dead_code)]
    pub fn refresh(&mut self) {
        self.commands = all_known_commands();
    }
}

impl Completer for YangzzHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        // ── @file path completion ──
        // Find the last @ before cursor that starts a file path token
        if let Some(result) = try_complete_file_path(line, pos) {
            return Ok(result);
        }

        // ── Slash command completion ──
        if !line.starts_with('/') {
            return Ok((pos, Vec::new()));
        }
        let first_ws = line.find(char::is_whitespace).unwrap_or(line.len());
        if pos > first_ws {
            return Ok((pos, Vec::new()));
        }
        let prefix = &line[..pos];

        let candidates: Vec<Pair> = self
            .commands
            .iter()
            .filter(|(n, _)| n.starts_with(prefix))
            .map(|(n, d)| Pair {
                display: format!("{n}  {d}"),
                replacement: n.clone(),
            })
            .collect();

        Ok((0, candidates))
    }
}

/// Try to complete a file path after @. Returns (start_pos, candidates) if applicable.
fn try_complete_file_path(line: &str, pos: usize) -> Option<(usize, Vec<Pair>)> {
    let before_cursor = &line[..pos];

    // Find the last @ that starts a file path token (preceded by whitespace or start of line)
    let at_pos = before_cursor.rfind('@')?;

    // @ must be at start of line or preceded by whitespace
    if at_pos > 0 && !line.as_bytes()[at_pos - 1].is_ascii_whitespace() {
        return None;
    }

    let partial = &before_cursor[at_pos + 1..];

    // Don't trigger on empty @ (user might be typing an email)
    // But do trigger on @./ or @src/ etc.
    if partial.is_empty() {
        // Show files in current directory
        let candidates = list_dir_entries(".", "");
        if candidates.is_empty() {
            return None;
        }
        return Some((at_pos + 1, candidates));
    }

    // Split into directory part and filename prefix
    let (dir, prefix) = if let Some(last_slash) = partial.rfind('/') {
        (&partial[..=last_slash], &partial[last_slash + 1..])
    } else {
        ("", partial)
    };

    let search_dir = if dir.is_empty() { "." } else { dir.trim_end_matches('/') };
    let candidates = list_dir_entries(search_dir, prefix);

    if candidates.is_empty() {
        return None;
    }

    // The replacement start position is after the @
    Some((at_pos + 1, candidates))
}

/// List directory entries matching a prefix
fn list_dir_entries(dir: &str, prefix: &str) -> Vec<Pair> {
    let dir_path = if dir == "." {
        std::env::current_dir().unwrap_or_default()
    } else {
        let p = Path::new(dir);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(p)
        }
    };

    let entries = match std::fs::read_dir(&dir_path) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let dir_prefix = if dir.is_empty() || dir == "." {
        String::new()
    } else {
        let mut d = dir.to_string();
        if !d.ends_with('/') {
            d.push('/');
        }
        d
    };

    let mut candidates: Vec<Pair> = entries
        .filter_map(|e| e.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files unless prefix starts with .
            if name.starts_with('.') && !prefix.starts_with('.') {
                return None;
            }

            // Filter by prefix
            if !prefix.is_empty() && !name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                return None;
            }

            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let display_name = if is_dir {
                format!("{name}/")
            } else {
                name.clone()
            };

            let replacement = if is_dir {
                format!("{dir_prefix}{name}/")
            } else {
                format!("{dir_prefix}{name}")
            };

            Some(Pair {
                display: display_name,
                replacement,
            })
        })
        .collect();

    candidates.sort_by(|a, b| a.display.cmp(&b.display));

    // Limit to 30 entries to avoid flooding
    candidates.truncate(30);
    candidates
}

impl Hinter for YangzzHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<String> {
        // Slash command hints
        if line.starts_with('/') && pos == line.len() {
            let first_ws = line.find(char::is_whitespace).unwrap_or(line.len());
            if pos <= first_ws {
                let typed = &line[..pos];
                return self
                    .commands
                    .iter()
                    .find(|(n, _)| n.starts_with(typed) && n.len() > typed.len())
                    .map(|(n, _)| n[typed.len()..].to_string());
            }
        }

        // @file path hints: show first matching file as ghost text
        if pos == line.len() {
            if let Some(at_pos) = line[..pos].rfind('@') {
                if at_pos == 0 || line.as_bytes()[at_pos - 1].is_ascii_whitespace() {
                    let partial = &line[at_pos + 1..];
                    if !partial.is_empty() {
                        let (dir, prefix) = if let Some(last_slash) = partial.rfind('/') {
                            (&partial[..=last_slash], &partial[last_slash + 1..])
                        } else {
                            ("", partial)
                        };
                        let search_dir = if dir.is_empty() { "." } else { dir.trim_end_matches('/') };
                        let candidates = list_dir_entries(search_dir, prefix);
                        if let Some(first) = candidates.first() {
                            // Show the remaining part as ghost hint
                            let full = format!("{}", first.replacement);
                            if full.len() > partial.len() && full.starts_with(partial) {
                                return Some(full[partial.len()..].to_string());
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

impl Highlighter for YangzzHelper {
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(format!("\x1b[2;37m{hint}\x1b[0m"))
    }
}

impl Validator for YangzzHelper {}

impl Helper for YangzzHelper {}
