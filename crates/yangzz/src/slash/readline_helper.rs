//! rustyline `Helper` that gives slash commands Tab-completion and ghost-hint
//! inline suggestions.
//!
//! How it feels to the user:
//! - Type `/pr` → right side shows dim `ovider` (ghost hint). Press right-arrow
//!   to accept, press Enter to send what you have.
//! - Type `/` and press Tab → lists all matches.
//! - Type `/prov` and press Tab → completes to `/provider`.
//!
//! Note: this is the "80%" version. A full persistent dropdown like Codex's
//! needs a custom crossterm input loop and is Phase 2 work.

use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};
use std::borrow::Cow;

use super::build_default;

/// The list of canonical slash commands + one-line summaries. Built from the
/// slash registry plus a few legacy commands that still live in repl.rs.
fn all_known_commands() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();

    // From registry
    let registry = build_default();
    for cmd in registry.all() {
        out.push((format!("/{}", cmd.name()), cmd.summary().to_string()));
    }

    // Legacy commands still handled outside the registry.
    const LEGACY: &[(&str, &str)] = &[];
    for (name, desc) in LEGACY {
        if !out.iter().any(|(n, _)| n == name) {
            out.push((name.to_string(), desc.to_string()));
        }
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

    /// Refresh the command list (e.g. after adding a new slash command at
    /// runtime — not common but cheap).
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
        // Only auto-complete at start of line on a slash command
        if !line.starts_with('/') {
            return Ok((pos, Vec::new()));
        }
        // Take the first token (up to whitespace) — we only complete the
        // command name itself, not its args.
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

impl Hinter for YangzzHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<String> {
        if !line.starts_with('/') || pos != line.len() {
            return None;
        }
        let first_ws = line.find(char::is_whitespace).unwrap_or(line.len());
        if pos > first_ws {
            return None;
        }
        let typed = &line[..pos];
        // Find the first command that extends what the user has typed.
        self.commands
            .iter()
            .find(|(n, _)| n.starts_with(typed) && n.len() > typed.len())
            .map(|(n, _)| n[typed.len()..].to_string())
    }
}

impl Highlighter for YangzzHelper {
    /// Render the hint dimly so it looks like ghost text.
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(format!("\x1b[2;37m{hint}\x1b[0m"))
    }
}

impl Validator for YangzzHelper {}

impl Helper for YangzzHelper {}
