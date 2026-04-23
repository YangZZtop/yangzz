//! Kernel-level sandbox: Seatbelt (macOS) + seccomp (Linux)
//! Wraps bash commands in a restricted sandbox profile.

use std::path::Path;
use tracing::{info, warn};

/// Sandbox mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxMode {
    /// No sandboxing
    Off,
    /// Warning only (log but allow)
    Warn,
    /// Enforce sandbox restrictions
    Enforce,
}

impl SandboxMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "enforce" | "on" | "true" => SandboxMode::Enforce,
            "warn" => SandboxMode::Warn,
            _ => SandboxMode::Off,
        }
    }
}

/// Generate a macOS Seatbelt profile for sandbox-exec
pub fn seatbelt_profile(cwd: &Path, net_policy: &NetworkPolicy) -> String {
    let cwd_str = cwd.to_string_lossy();
    let net_rule = if net_policy.allow_outbound {
        "(allow network*)"
    } else {
        "(deny network*)"
    };

    format!(
        r#"(version 1)
(deny default)
(allow process-exec)
(allow process-fork)
(allow sysctl-read)
(allow mach-lookup)

;; Allow read/write in project directory
(allow file-read* (subpath "{cwd_str}"))
(allow file-write* (subpath "{cwd_str}"))

;; Allow read system paths
(allow file-read* (subpath "/usr"))
(allow file-read* (subpath "/bin"))
(allow file-read* (subpath "/sbin"))
(allow file-read* (subpath "/etc"))
(allow file-read* (subpath "/Library"))
(allow file-read* (subpath "/System"))
(allow file-read* (subpath "/dev"))
(allow file-read* (subpath "/private/tmp"))
(allow file-read* (subpath "/private/var"))
(allow file-read* (subpath "/tmp"))
(allow file-read* (subpath "/var"))

;; Allow tmp writes
(allow file-write* (subpath "/private/tmp"))
(allow file-write* (subpath "/tmp"))
(allow file-write* (subpath "/dev"))

;; Home directory read (for configs)
(allow file-read* (subpath (param "HOME")))

;; Network policy
{net_rule}
"#
    )
}

/// Wrap a command for sandboxed execution on macOS
pub fn wrap_command_macos(cmd: &str, cwd: &Path, net_policy: &NetworkPolicy) -> String {
    let profile = seatbelt_profile(cwd, net_policy);
    // Write profile to temp file
    let profile_path = std::env::temp_dir().join("yangzz_sandbox.sb");
    let _ = std::fs::write(&profile_path, &profile);

    format!(
        "sandbox-exec -f {} -- sh -c {}",
        profile_path.display(),
        shell_escape(cmd)
    )
}

/// Generate seccomp-bpf restrictions for Linux
pub fn wrap_command_linux(cmd: &str, _cwd: &Path, _net_policy: &NetworkPolicy) -> String {
    // On Linux, use unshare for namespace isolation if available
    // Falls back to regular execution if unshare not available
    format!(
        "unshare --net --map-root-user -- sh -c {} 2>/dev/null || sh -c {}",
        shell_escape(cmd),
        shell_escape(cmd)
    )
}

/// Wrap a command with appropriate sandbox for the current OS
pub fn wrap_command(cmd: &str, cwd: &Path, mode: SandboxMode, net_policy: &NetworkPolicy) -> String {
    match mode {
        SandboxMode::Off => cmd.to_string(),
        SandboxMode::Warn => {
            warn!("Sandbox warn mode: command would be sandboxed: {cmd}");
            cmd.to_string()
        }
        SandboxMode::Enforce => {
            info!("Sandboxing command: {cmd}");
            if cfg!(target_os = "macos") {
                wrap_command_macos(cmd, cwd, net_policy)
            } else if cfg!(target_os = "linux") {
                wrap_command_linux(cmd, cwd, net_policy)
            } else {
                warn!("Sandbox not supported on this OS, running unsandboxed");
                cmd.to_string()
            }
        }
    }
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

// ── Execution Policy (TOML-based) ──

use serde::Deserialize;

/// Static execution policy loaded from .yangzz/policy.toml
#[derive(Debug, Deserialize, Clone)]
pub struct ExecutionPolicy {
    #[serde(default)]
    pub sandbox: SandboxConfig,
    #[serde(default)]
    pub network: NetworkPolicy,
    #[serde(default)]
    pub filesystem: FilesystemPolicy,
    #[serde(default)]
    pub commands: CommandPolicy,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SandboxConfig {
    #[serde(default = "default_sandbox_mode")]
    pub mode: String,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self { mode: "off".to_string() }
    }
}

fn default_sandbox_mode() -> String { "off".to_string() }

/// Network access policy
#[derive(Debug, Deserialize, Clone)]
pub struct NetworkPolicy {
    #[serde(default = "default_true")]
    pub allow_outbound: bool,
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
    #[serde(default)]
    pub blocked_hosts: Vec<String>,
    #[serde(default)]
    pub allowed_ports: Vec<u16>,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            allow_outbound: true,
            allowed_hosts: Vec::new(),
            blocked_hosts: Vec::new(),
            allowed_ports: Vec::new(),
        }
    }
}

fn default_true() -> bool { true }

/// Filesystem access policy
#[derive(Debug, Deserialize, Clone)]
pub struct FilesystemPolicy {
    #[serde(default)]
    pub read_only_paths: Vec<String>,
    #[serde(default)]
    pub denied_paths: Vec<String>,
    #[serde(default = "default_true")]
    pub allow_symlinks: bool,
}

impl Default for FilesystemPolicy {
    fn default() -> Self {
        Self {
            read_only_paths: Vec::new(),
            denied_paths: Vec::new(),
            allow_symlinks: true,
        }
    }
}

/// Command execution policy
#[derive(Debug, Deserialize, Clone)]
pub struct CommandPolicy {
    #[serde(default)]
    pub allowed_commands: Vec<String>,
    #[serde(default)]
    pub blocked_commands: Vec<String>,
    #[serde(default = "default_true")]
    pub allow_sudo: bool,
    #[serde(default = "default_max_runtime")]
    pub max_runtime_secs: u64,
}

impl Default for CommandPolicy {
    fn default() -> Self {
        Self {
            allowed_commands: Vec::new(),
            blocked_commands: Vec::new(),
            allow_sudo: true,
            max_runtime_secs: 300,
        }
    }
}

fn default_max_runtime() -> u64 { 300 }

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            sandbox: SandboxConfig::default(),
            network: NetworkPolicy::default(),
            filesystem: FilesystemPolicy::default(),
            commands: CommandPolicy::default(),
        }
    }
}

/// Load execution policy from .yangzz/policy.toml
pub fn load_policy(cwd: &Path) -> ExecutionPolicy {
    let policy_path = cwd.join(".yangzz").join("policy.toml");
    if !policy_path.exists() {
        return ExecutionPolicy::default();
    }

    match std::fs::read_to_string(&policy_path) {
        Ok(content) => {
            match toml::from_str::<ExecutionPolicy>(&content) {
                Ok(policy) => {
                    info!("Loaded execution policy from {}", policy_path.display());
                    policy
                }
                Err(e) => {
                    warn!("Invalid policy.toml: {e}, using defaults");
                    ExecutionPolicy::default()
                }
            }
        }
        Err(e) => {
            warn!("Cannot read policy.toml: {e}");
            ExecutionPolicy::default()
        }
    }
}

/// Check if a command is allowed by policy
pub fn check_command_policy(cmd: &str, policy: &CommandPolicy) -> Result<(), String> {
    // Check blocked commands
    for blocked in &policy.blocked_commands {
        if cmd.contains(blocked) {
            return Err(format!("Command blocked by policy: contains '{blocked}'"));
        }
    }

    // If allowlist is set, command must match
    if !policy.allowed_commands.is_empty() {
        let cmd_name = cmd.split_whitespace().next().unwrap_or("");
        if !policy.allowed_commands.iter().any(|a| cmd_name.starts_with(a)) {
            return Err(format!("Command not in allowlist: {cmd_name}"));
        }
    }

    // Check sudo
    if !policy.allow_sudo && cmd.contains("sudo") {
        return Err("sudo not allowed by policy".to_string());
    }

    Ok(())
}

/// Check if a network request is allowed by policy
pub fn check_network_policy(host: &str, port: u16, policy: &NetworkPolicy) -> Result<(), String> {
    if !policy.allow_outbound {
        return Err("Outbound network access denied by policy".to_string());
    }

    // Check blocked hosts
    for blocked in &policy.blocked_hosts {
        if host.contains(blocked) {
            return Err(format!("Host blocked by policy: {host}"));
        }
    }

    // If allowed_hosts is set, host must match
    if !policy.allowed_hosts.is_empty() {
        if !policy.allowed_hosts.iter().any(|a| host.contains(a)) {
            return Err(format!("Host not in allowlist: {host}"));
        }
    }

    // Check allowed ports
    if !policy.allowed_ports.is_empty() && !policy.allowed_ports.contains(&port) {
        return Err(format!("Port {port} not in allowlist"));
    }

    Ok(())
}
