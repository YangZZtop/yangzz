use std::collections::HashSet;
use std::io::{self, Write};
use tokio::sync::Mutex;

/// Permission manager — smart auto-pass for safe commands, ask for others
pub struct PermissionManager {
    always_allow: Mutex<HashSet<String>>,
    auto_approve: bool,
}

impl PermissionManager {
    pub fn new() -> Self {
        Self {
            always_allow: Mutex::new(HashSet::new()),
            auto_approve: false,
        }
    }

    /// Create a permission manager that auto-approves everything (for TUI mode)
    pub fn auto_approve() -> Self {
        Self {
            always_allow: Mutex::new(HashSet::new()),
            auto_approve: true,
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
        // 0. Auto-approve mode (TUI)
        if self.auto_approve {
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

        // 3. Interactive prompt
        let action = if is_destructive { "Write" } else { "Write" };
        eprint!(
            "\n  \x1b[33m●\x1b[0m {action} tool \x1b[1m{tool_name}\x1b[0m — \x1b[2m[y]es / [n]o / [a]lways\x1b[0m: "
        );
        let _ = io::stderr().flush();

        let mut response = String::new();
        io::stdin().read_line(&mut response).map_err(|e| e.to_string())?;

        match response.trim().to_lowercase().as_str() {
            "y" | "yes" | "" => Ok(true),
            "a" | "always" => {
                let mut allowed = self.always_allow.lock().await;
                allowed.insert(tool_name.to_string());
                Ok(true)
            }
            _ => Ok(false),
        }
    }
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
        "ls", "ll", "la", "dir",
        "cat", "head", "tail", "less", "more",
        "find", "fd",
        "grep", "rg", "ag", "ack",
        "wc", "sort", "uniq", "diff", "comm",
        "pwd", "realpath", "dirname", "basename",
        "echo", "printf",
        "date", "cal",
        "whoami", "id", "hostname", "uname",
        "which", "type", "where", "command",
        "file", "stat", "du", "df",
        "env", "printenv",
        "git", // git is mostly safe (status, log, branch, diff)
        "tree",
        "jq", "yq",
        "node", "python", "python3", "ruby",
        "cargo",
    ];

    if SAFE_COMMANDS.contains(&first_word) {
        // Extra safety: reject git push/reset/clean
        if first_word == "git" {
            let rest = trimmed.strip_prefix("git").unwrap_or("").trim();
            let sub = rest.split_whitespace().next().unwrap_or("");
            let dangerous_git = ["push", "reset", "clean", "checkout", "rebase", "merge", "stash"];
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
        ":(){ :|:& };:",   // fork bomb
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
    ];
    for p in &patterns {
        if lower.contains(p) { return true; }
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
        (r"eyJ[a-zA-Z0-9_-]{20,}\.[a-zA-Z0-9_-]{20,}\.[a-zA-Z0-9_-]{20,}", "JWT token"),
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
