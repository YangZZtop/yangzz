use std::collections::HashSet;
use std::io::{self, Write};
use tokio::sync::{Mutex, mpsc, oneshot};

/// How permission prompts are answered.
pub enum PermissionMode {
    /// Ask via stdin prompt (REPL mode).
    Cli,
    /// Auto-approve everything (used as a last-resort fallback).
    Auto,
    /// Send asks over a channel; a UI (TUI) answers on a oneshot. This keeps
    /// ratatui's raw-mode rendering intact — no stdin interference.
    Channel(mpsc::UnboundedSender<PermissionAsk>),
}

/// Message sent from PermissionManager to a Channel-mode receiver (the TUI).
pub struct PermissionAsk {
    pub tool_name: String,
    pub input: serde_json::Value,
    pub is_destructive: bool,
    pub reply: oneshot::Sender<PermissionAnswer>,
}

#[derive(Debug, Clone, Copy)]
pub enum PermissionAnswer {
    Yes,
    No,
    Always,
}

/// Permission manager — smart auto-pass for safe commands, ask for others
pub struct PermissionManager {
    always_allow: Mutex<HashSet<String>>,
    mode: PermissionMode,
}

impl PermissionManager {
    pub fn new() -> Self {
        Self {
            always_allow: Mutex::new(HashSet::new()),
            mode: PermissionMode::Cli,
        }
    }

    /// Create a permission manager that auto-approves everything (fallback).
    pub fn auto_approve() -> Self {
        Self {
            always_allow: Mutex::new(HashSet::new()),
            mode: PermissionMode::Auto,
        }
    }

    /// Create a permission manager that asks via a channel — used by TUI so
    /// prompts appear as a modal dialog instead of disturbing ratatui.
    pub fn channel(tx: mpsc::UnboundedSender<PermissionAsk>) -> Self {
        Self {
            always_allow: Mutex::new(HashSet::new()),
            mode: PermissionMode::Channel(tx),
        }
    }

    /// Check if a tool invocation is allowed.
    /// Returns Ok(true) for allowed, Ok(false) for denied.
    pub async fn check(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
        is_destructive: bool,
    ) -> Result<bool, String> {
        // 0. Auto-approve mode — bypass everything.
        if matches!(self.mode, PermissionMode::Auto) {
            return Ok(true);
        }

        // 1. If user chose "always" for this tool, auto-pass (even destructive)
        {
            let allowed = self.always_allow.lock().await;
            if allowed.contains(tool_name) {
                return Ok(true);
            }
        }

        // 2. For bash: auto-pass safe read-only commands
        if tool_name == "bash" {
            if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                if is_safe_bash_command(cmd) {
                    return Ok(true);
                }
            }
        }

        // 3. Interactive prompt (mode-dependent)
        let answer = match &self.mode {
            PermissionMode::Auto => PermissionAnswer::Yes, // handled above
            PermissionMode::Cli => ask_via_stdin(tool_name)?,
            PermissionMode::Channel(tx) => {
                ask_via_channel(tx, tool_name.to_string(), input.clone(), is_destructive).await?
            }
        };

        match answer {
            PermissionAnswer::Yes => Ok(true),
            PermissionAnswer::Always => {
                let mut allowed = self.always_allow.lock().await;
                allowed.insert(tool_name.to_string());
                Ok(true)
            }
            PermissionAnswer::No => Ok(false),
        }
    }
}

fn ask_via_stdin(tool_name: &str) -> Result<PermissionAnswer, String> {
    eprint!(
        "\n  \x1b[33m●\x1b[0m Write tool \x1b[1m{tool_name}\x1b[0m — \x1b[2m[y]es / [n]o / [a]lways\x1b[0m: "
    );
    let _ = io::stderr().flush();

    let mut response = String::new();
    io::stdin()
        .read_line(&mut response)
        .map_err(|e| e.to_string())?;

    Ok(match response.trim().to_lowercase().as_str() {
        "y" | "yes" | "" => PermissionAnswer::Yes,
        "a" | "always" => PermissionAnswer::Always,
        _ => PermissionAnswer::No,
    })
}

async fn ask_via_channel(
    tx: &mpsc::UnboundedSender<PermissionAsk>,
    tool_name: String,
    input: serde_json::Value,
    is_destructive: bool,
) -> Result<PermissionAnswer, String> {
    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(PermissionAsk {
        tool_name,
        input,
        is_destructive,
        reply: reply_tx,
    })
    .map_err(|e| format!("permission channel closed: {e}"))?;
    reply_rx
        .await
        .map_err(|e| format!("permission reply dropped: {e}"))
}

/// Detect if a bash command is safe (read-only, no side effects)
fn is_safe_bash_command(cmd: &str) -> bool {
    let trimmed = cmd.trim();

    // DANGEROUS COMMAND DETECTION — block outright even with "always allow"
    if is_dangerous_bash_command(trimmed) {
        return false;
    }

    // Get the first word (the actual command)
    let first_word = trimmed
        .split(|c: char| c.is_whitespace() || c == '|' || c == ';' || c == '&')
        .next()
        .unwrap_or("");

    // These commands are always safe (read-only)
    const SAFE_COMMANDS: &[&str] = &[
        "ls",
        "ll",
        "la",
        "dir",
        "cat",
        "head",
        "tail",
        "less",
        "more",
        "find",
        "fd",
        "grep",
        "rg",
        "ag",
        "ack",
        "wc",
        "sort",
        "uniq",
        "diff",
        "comm",
        "pwd",
        "realpath",
        "dirname",
        "basename",
        "echo",
        "printf",
        "date",
        "cal",
        "whoami",
        "id",
        "hostname",
        "uname",
        "which",
        "type",
        "where",
        "command",
        "file",
        "stat",
        "du",
        "df",
        "env",
        "printenv",
        "git", // git is mostly safe (status, log, branch, diff)
        "tree",
        "jq",
        "yq",
        "node",
        "python",
        "python3",
        "ruby",
        "cargo",
        // Windows-specific safe commands
        "type",    // Windows cat
        "findstr", // Windows grep
        "where",   // Windows which
        "systeminfo",
        "ver",
        "set", // show env vars
        "path",
        "chdir",
        "cd",
    ];

    if SAFE_COMMANDS.contains(&first_word) {
        // Extra safety: reject git push/reset/clean
        if first_word == "git" {
            let rest = trimmed.strip_prefix("git").unwrap_or("").trim();
            let sub = rest.split_whitespace().next().unwrap_or("");
            let dangerous_git = [
                "push", "reset", "clean", "checkout", "rebase", "merge", "stash",
            ];
            if dangerous_git.contains(&sub) {
                return false;
            }
        }
        // Extra safety: reject cargo install/publish
        if first_word == "cargo" {
            let rest = trimmed.strip_prefix("cargo").unwrap_or("").trim();
            let sub = rest.split_whitespace().next().unwrap_or("");
            let dangerous_cargo = ["install", "publish", "uninstall"];
            if dangerous_cargo.contains(&sub) {
                return false;
            }
        }
        return true;
    }

    // Commands that start with these are safe
    if trimmed.starts_with("test ") || trimmed.starts_with("[ ") {
        return true;
    }

    false
}

/// Detect highly dangerous bash commands that should always warn
fn is_dangerous_bash_command(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    let patterns = [
        "rm -rf /",
        "rm -rf ~",
        "rm -rf $home",
        "mkfs.",
        "dd if=",
        "> /dev/sd",
        ":(){ :|:& };:", // fork bomb
        "chmod -r 777 /",
        "chmod -r 000 /",
        "curl|sh",
        "curl|bash",
        "wget|sh",
        "wget|bash",
        "git push --force",
        "git push -f ",
        "git reset --hard",
        "drop table",
        "drop database",
        "truncate table",
        "delete from",
        "format c:",
        "shutdown",
        "reboot",
        "init 0",
        "init 6",
        "kill -9 1",
        "pkill -9",
        "killall",
        // Windows-specific dangerous commands
        "del /f /s /q c:\\",
        "rd /s /q c:\\",
        "rmdir /s /q c:\\",
        "del /f /s /q %systemroot%",
        "format d:",
        "format e:",
        "diskpart",
        "bcdedit",
        "reg delete",
        "taskkill /f /im",
        "net stop",
        "sc delete",
    ];
    for p in &patterns {
        if lower.contains(p) {
            return true;
        }
    }
    false
}

/// Check if tool output contains leaked secrets
pub fn scan_for_secrets(text: &str) -> Vec<String> {
    let mut findings = Vec::new();
    let patterns: &[(&str, &str)] = &[
        (r"sk-[a-zA-Z0-9]{20,}", "OpenAI API key"),
        (r"sk-ant-[a-zA-Z0-9\-]{20,}", "Anthropic API key"),
        (r"ghp_[a-zA-Z0-9]{36,}", "GitHub personal access token"),
        (r"gho_[a-zA-Z0-9]{36,}", "GitHub OAuth token"),
        (r"AKIA[0-9A-Z]{16}", "AWS access key ID"),
        (
            r"eyJ[a-zA-Z0-9_-]{20,}\.[a-zA-Z0-9_-]{20,}\.[a-zA-Z0-9_-]{20,}",
            "JWT token",
        ),
        (r"xox[bpsa]-[a-zA-Z0-9\-]{10,}", "Slack token"),
    ];
    for (pat, label) in patterns {
        if let Ok(re) = regex::Regex::new(pat) {
            if re.is_match(text) {
                findings.push(format!("⚠ Possible {label} detected in output"));
            }
        }
    }
    findings
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}
