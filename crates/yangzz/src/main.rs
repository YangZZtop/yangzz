mod guide;
mod repl;
mod repl_commands;
mod repl_help;
mod repl_render;
mod slash;
mod tui;
mod ui;

use clap::Parser;
use std::sync::Arc;
use yangzz_core::config::settings::CliOverrides;
use yangzz_core::config::{self, Settings};
use yangzz_core::permission::PermissionManager;
use yangzz_core::tool::{ToolExecutor, ToolRegistry};

pub use guide::print_guide;
use ui::format::*;

#[derive(Parser)]
#[command(name = "yangzz")]
#[command(about = "AI coding assistant — any model, one variable, ready to go")]
#[command(version)]
struct Cli {
    /// Initial prompt (if provided, runs in single-shot mode)
    prompt: Option<String>,

    /// Provider name (anthropic, openai, gemini, deepseek, glm, grok, xiaomi, ollama)
    #[arg(long)]
    provider: Option<String>,

    /// Model to use
    #[arg(long, short)]
    model: Option<String>,

    /// API key (prefer env var instead)
    #[arg(long)]
    api_key: Option<String>,

    /// Custom API base URL
    #[arg(long)]
    base_url: Option<String>,

    /// Run first-time setup wizard
    #[arg(long)]
    setup: bool,

    /// Run classic REPL mode (default since v0.3.1 — matches README).
    /// Kept as a no-op flag so `yangzz --repl` from old scripts still works.
    #[arg(long, alias = "legacy")]
    repl: bool,

    /// Opt into experimental full-screen TUI (alternate screen + modal
    /// permission dialogs). REPL is recommended for daily use because
    /// it preserves native terminal scroll / selection / copy.
    #[arg(long)]
    tui: bool,

    /// Show quick-start guide
    #[arg(long)]
    guide: bool,

    /// Run health check (config, provider, tools)
    #[arg(long)]
    doctor: bool,

    /// Uninstall yangzz: remove config, sessions, memory (with confirmation)
    #[arg(long)]
    uninstall: bool,

    /// Print all yangzz data paths (useful for debugging / manual cleanup)
    #[arg(long = "where")]
    where_paths: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Init logging — only show on RUST_LOG, silent by default
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive("yangzz=warn".parse()?),
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    // Windows: enable ANSI escape codes in cmd.exe / PowerShell
    #[cfg(target_os = "windows")]
    {
        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::SetTitle("yangzz"));
        // Enable virtual terminal processing for ANSI color support
        let _ = crossterm::terminal::enable_raw_mode();
        let _ = crossterm::terminal::disable_raw_mode();
    }

    let cli = Cli::parse();

    // Silently migrate legacy data (pre-v0.3.0) on every startup.
    // Best-effort; safe to run repeatedly.
    if yangzz_core::paths::maybe_migrate_legacy() {
        println!(
            "  {DIM}已自动迁移旧版数据 → {}{RESET}",
            yangzz_core::paths::yangzz_dir().display()
        );
    }

    // --where flag: print all data paths
    if cli.where_paths {
        print_where();
        return Ok(());
    }

    // --uninstall flag: interactive uninstall
    if cli.uninstall {
        run_uninstall();
        return Ok(());
    }

    // --guide flag: show quick-start guide
    if cli.guide {
        print_guide();
        return Ok(());
    }

    // --setup flag: always run wizard
    if cli.setup {
        if !run_setup_wizard() {
            return Ok(());
        }
        // Wizard succeeded — fall through to start REPL
    }

    // --doctor flag: health check
    if cli.doctor {
        run_doctor();
        return Ok(());
    }

    // Load settings with CLI overrides
    let settings = Settings::load(CliOverrides {
        provider: cli.provider,
        model: cli.model,
        api_key: cli.api_key,
        base_url: cli.base_url,
    });

    // Resolve provider — on failure, offer guided setup
    let provider = match config::resolve_provider(&settings) {
        Ok(p) => p,
        Err(_e) => {
            if run_setup_wizard() {
                // Reload settings after wizard wrote config
                let new_settings = Settings::load(CliOverrides::default());
                match config::resolve_provider(&new_settings) {
                    Ok(p) => p,
                    Err(e2) => {
                        println!("  {RED}✖{RESET} 配置后仍无法连接: {e2}");
                        println!("    {DIM}请检查配置文件或运行 yangzz --doctor{RESET}");
                        std::process::exit(1);
                    }
                }
            } else {
                std::process::exit(0);
            }
        }
    };

    // Re-load settings in case wizard just wrote a new config
    let settings = if cli.setup {
        Settings::load(CliOverrides::default())
    } else {
        settings
    };

    let model = settings.resolved_model();
    let max_tokens = settings.resolved_max_tokens();

    // Setup tool system
    let cwd = std::env::current_dir()?;
    let registry = ToolRegistry::with_builtins(&cwd);

    if let Some(prompt) = cli.prompt {
        // Single-shot mode — REPL-style rendering to stdout makes sense for
        // pipelines (yangzz "explain" | tee out.txt).
        let permission = Arc::new(PermissionManager::new());
        let executor = ToolExecutor::new(registry, permission, cwd);
        repl::single_shot(&provider, &model, max_tokens, &prompt, &executor).await?;
    } else if cli.tui {
        // Opt-in: experimental TUI (alternate-screen + modal permissions).
        // Trade-off: loses native terminal scroll / selection / copy.
        let (perm_tx, perm_rx) = tokio::sync::mpsc::unbounded_channel();
        let permission = Arc::new(PermissionManager::channel(perm_tx));
        let executor = ToolExecutor::new(registry, permission, cwd);
        tui::run(
            &provider,
            &model,
            max_tokens,
            Arc::new(executor),
            &settings,
            perm_rx,
        )
        .await?;
    } else {
        // Default (v0.3.1+): classic REPL — the documented product.
        // Native scroll, native selection, full slash-command fidelity.
        // `--repl` is now a no-op but kept so existing scripts don't break.
        let _ = cli.repl;
        let permission = Arc::new(PermissionManager::new());
        let executor = ToolExecutor::new(registry, permission, cwd);
        repl::run(&provider, &model, max_tokens, &executor, &settings).await?;
    }

    Ok(())
}

// ── Interactive Setup Wizard ──
// 核心心智模型（基于 cc-switch / sub2api / new-api 三项目分析）：
//   - provider  = 你给这份配置起的名字（不是厂商）
//   - base_url  = 实际入口地址（允许带路径前缀，如 /antigravity）
//   - api_format = 入口用什么协议说话（openai / anthropic / gemini / auto）
//   - model     = 希望上游路由到的模型名
// 只问必要问题，其他给合理默认。

/// Returns true if setup succeeded and caller should continue into REPL
fn run_setup_wizard() -> bool {
    use std::io::{self, Write};

    fn prompt(msg: &str) -> String {
        print!("  {msg}");
        io::stdout().flush().unwrap();
        let mut buf = String::new();
        if io::stdin().read_line(&mut buf).is_err() {
            return String::new();
        }
        buf.trim().to_string()
    }

    fn prompt_default(msg: &str, default: &str) -> String {
        print!("  {msg} {DIM}[{default}]{RESET}: ");
        io::stdout().flush().unwrap();
        let mut buf = String::new();
        if io::stdin().read_line(&mut buf).is_err() {
            return default.to_string();
        }
        let val = buf.trim().to_string();
        if val.is_empty() {
            default.to_string()
        } else {
            val
        }
    }

    // ── Banner ──
    println!();
    println!("  {BOLD_GOLD} █████ ████  ██████   ████████    ███████  █████████  █████████{RESET}");
    println!("  {BOLD_GOLD} ░░███ ░███  ░░░░░███ ░░███░░███  ███░░███ ░█░░░░███  ░█░░░░███{RESET}");
    println!("  {BOLD_GOLD}  ░███ ░███   ███████  ░███ ░███ ░███ ░███ ░   ███░   ░   ███░{RESET}");
    println!(
        "  {BOLD_GOLD}  ░███ ░███  ███░░███  ░███ ░███ ░███ ░███   ███░   █   ███░   █{RESET}"
    );
    println!(
        "  {BOLD_GOLD}  ░░███████ ░░████████ ████ █████░░███████  █████████  █████████{RESET}"
    );
    println!("  {BOLD_GOLD}   ░░░░░███  ░░░░░░░░ ░░░░ ░░░░░  ░░░░░███ ░░░░░░░░░  ░░░░░░░░░{RESET}");
    println!("  {BOLD_GOLD}   ███ ░███                       ███ ░███{RESET}");
    println!("  {BOLD_GOLD}  ░░██████                       ░░██████{RESET}");
    println!("  {BOLD_GOLD}   ░░░░░░                         ░░░░░░{RESET}");
    println!();
    println!(
        "  {BOLD}欢迎使用 yangzz！{RESET} {DIM}下面 4 步给这份配置起名、填入口、填 Key、选模型。{RESET}"
    );
    println!();

    // ── 1. 给配置起名 ──
    // provider 就是一个 profile 名，不是厂商。
    let name = prompt_default("给这份配置起个名字", "my-relay");
    if name.is_empty() {
        return false;
    }

    // ── 2. 入口地址 ──
    // 允许带 path（例如 https://relay.example.com/antigravity）
    let url = prompt(&format!(
        "{BOLD}入口地址{RESET} {DIM}(含路径，如 https://api.example.com 或 https://relay.com/antigravity): {RESET}"
    ));
    if url.is_empty() {
        println!("  {RED}✖{RESET} 入口地址不能为空");
        return false;
    }

    // ── 3. API Key ──
    let key = prompt(&format!("{BOLD}API Key: {RESET}"));
    if key.is_empty() {
        println!("  {RED}✖{RESET} Key 不能为空");
        return false;
    }

    // ── 4. 默认模型 ──
    let model = prompt_default(
        "默认模型 (要跟入口实际支持的模型名一致)",
        "claude-sonnet-4-20250514",
    );

    // ── api_format：入口协议 ──
    // 99% 的中转站都说 OpenAI 协议，默认就够。
    // auto 表示「按 URL host 自动判断」—— 对官方域名有效，对自定义中转仍按 OpenAI。
    // 如果你的中转只提供 Claude 原生 /v1/messages，进配置文件手动改成 "anthropic" 即可。
    let api_format = "openai";

    // ── Write config — unified ~/.yangzz/config.toml ──
    yangzz_core::paths::ensure_yangzz_dir();
    let dir = yangzz_core::paths::yangzz_dir();
    let config_path = yangzz_core::paths::config_path();

    if config_path.exists() {
        let overwrite = prompt_default(&format!("{YELLOW}⚠{RESET} 配置已存在，覆盖？(y/n)"), "n");
        if !overwrite.to_lowercase().starts_with('y') {
            println!("  已取消。");
            return false;
        }
    }

    if let Err(e) = std::fs::create_dir_all(&dir) {
        println!("  {RED}✖{RESET} 创建目录失败: {e}");
        return false;
    }

    let toml = format!(
        r#"provider = "{name}"
model = "{model}"

[[providers]]
name = "{name}"
api_key = "{key}"
base_url = "{url}"
default_model = "{model}"
api_format = "{api_format}"
"#
    );

    if let Err(e) = std::fs::write(&config_path, &toml) {
        println!("  {RED}✖{RESET} 写入失败: {e}");
        return false;
    }

    println!();
    println!(
        "  {GREEN}✓{RESET} 配置已保存 {DIM}{}{RESET}",
        config_path.display()
    );
    println!();
    println!("  {BOLD}下面几个命令马上就能用：{RESET}");
    println!("    {GOLD}输入你的问题{RESET}       {DIM}直接打字，回车发送{RESET}");
    println!("    {GOLD}/help{RESET}             {DIM}查看所有命令{RESET}");
    println!("    {GOLD}/provider add{RESET}     {DIM}再加一个中转{RESET}");
    println!("    {GOLD}/model{RESET}            {DIM}切换模型{RESET}");
    println!("    {GOLD}/quit{RESET}             {DIM}退出{RESET}");
    println!();

    true
}

// ── Health Check (yangzz --doctor) ──

fn run_doctor() {
    println!();
    println!("  {BOLD}yangzz --doctor{RESET}  Health Check");
    println!("  ══════════════════════════");
    println!();

    let mut passed = 0u32;
    let warned = 0u32;
    let mut failed = 0u32;

    // 1. Config file — unified ~/.yangzz/
    let settings = Settings::load(CliOverrides::default());
    let config_path = yangzz_core::paths::config_path();

    if config_path.exists() {
        println!("  {GREEN}✓{RESET} Config file: {}", config_path.display());
        passed += 1;
    } else {
        // Check project-local fallback
        let cwd = std::env::current_dir().unwrap_or_default();
        let local = cwd.join(".yangzz.toml");
        if local.exists() {
            println!(
                "  {GREEN}✓{RESET} Config file: {} (project-local)",
                local.display()
            );
            passed += 1;
        } else {
            println!(
                "  {RED}✖{RESET} Config file not found: {}",
                config_path.display()
            );
            println!("    {DIM}Run: yangzz --setup{RESET}");
            failed += 1;
        }
    }

    // 2. Provider
    if settings.provider.is_some() {
        match config::resolve_provider(&settings) {
            Ok(p) => {
                println!(
                    "  {GREEN}✓{RESET} Provider: {} ({})",
                    p.name(),
                    settings.resolved_model()
                );
                passed += 1;
            }
            Err(e) => {
                println!("  {RED}✖{RESET} Provider resolution failed: {e}");
                failed += 1;
            }
        }
    } else {
        println!("  {RED}✖{RESET} No provider configured");
        println!("    {DIM}Add 'provider = \"...\"' to config.toml{RESET}");
        failed += 1;
    }

    // 3. API Key
    if settings.api_key.is_some() || !settings.providers.is_empty() {
        let has_key =
            settings.api_key.is_some() || settings.providers.iter().any(|p| !p.api_key.is_empty());
        let active = settings.provider.as_deref().unwrap_or("");
        let allows_keyless = settings
            .providers
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case(active) && p.api_key.is_empty())
            || config::PRESETS.iter().any(|preset| {
                preset.name.eq_ignore_ascii_case(active) && preset.api_key_env.is_empty()
            });

        if has_key {
            println!("  {GREEN}✓{RESET} API key configured");
            passed += 1;
        } else if allows_keyless {
            println!("  {GREEN}✓{RESET} Current provider does not require an API key");
            passed += 1;
        } else {
            println!("  {RED}✖{RESET} No API key found");
            failed += 1;
        }
    } else {
        // Check env vars
        let env_keys = [
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "GEMINI_API_KEY",
            "DEEPSEEK_API_KEY",
        ];
        let has_env = env_keys.iter().any(|k| std::env::var(k).is_ok());
        if has_env {
            println!("  {GREEN}✓{RESET} API key (from environment variable)");
            passed += 1;
        } else {
            println!("  {RED}✖{RESET} No API key found (config or env)");
            failed += 1;
        }
    }

    // 4. Extra providers
    let mut unique_providers = Vec::new();
    for ep in &settings.providers {
        if unique_providers.iter().any(
            |existing: &&yangzz_core::config::settings::ExtraProvider| {
                existing.name.eq_ignore_ascii_case(&ep.name)
            },
        ) {
            continue;
        }
        unique_providers.push(ep);
    }

    let provider_count = unique_providers.len();
    if provider_count > 0 {
        println!("  {GREEN}✓{RESET} Extra providers: {provider_count} configured");
        for ep in unique_providers {
            let model = ep.default_model.as_deref().unwrap_or("(default)");
            println!(
                "    {DIM}• {} → {} [{}]{RESET}",
                ep.name, ep.base_url, model
            );
        }
        passed += 1;
    } else {
        println!("  {DIM}─{RESET} No extra [[providers]] (optional)");
    }

    // 5. Rust toolchain
    let has_cargo = std::process::Command::new("cargo")
        .arg("--version")
        .output()
        .is_ok();
    if has_cargo {
        println!("  {GREEN}✓{RESET} Rust toolchain available");
        passed += 1;
    } else {
        println!("  {DIM}─{RESET} Rust toolchain not found (optional, for cargo install)");
    }

    // 6. Working directory
    let cwd = std::env::current_dir().unwrap_or_default();
    let has_git = cwd.join(".git").exists();
    let has_memory = cwd.join("MEMORY.md").exists();
    println!("  {DIM}─{RESET} Working dir: {}", cwd.display());
    if has_git {
        println!("    {GREEN}✓{RESET} Git repo detected");
        passed += 1;
    }
    if has_memory {
        println!("    {GREEN}✓{RESET} MEMORY.md found");
        passed += 1;
    }

    // 7. Shell (Windows check)
    if cfg!(target_os = "windows") {
        println!("  {GREEN}✓{RESET} Platform: Windows (cmd.exe/PowerShell)");
        passed += 1;
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "unknown".into());
        println!("  {GREEN}✓{RESET} Shell: {shell}");
        passed += 1;
    }

    // Summary
    println!();
    if failed == 0 {
        println!(
            "  {GREEN}■{RESET} {BOLD}All checks passed{RESET} ({passed} ok, {warned} warnings)"
        );
    } else {
        println!(
            "  {RED}■{RESET} {BOLD}{failed} issue(s) found{RESET} ({passed} ok, {warned} warnings)"
        );
        println!("    {DIM}Fix the issues above, then run 'yangzz --doctor' again{RESET}");
    }
    println!();
}

// ── `yangzz --where` ──
// Prints every yangzz data path so users can inspect / manually clean.

fn print_where() {
    println!();
    println!("  {BOLD}yangzz 数据位置{RESET}");
    println!("  ══════════════════════════");
    println!();
    println!("  {BOLD}主目录：{RESET}");
    println!(
        "    {GOLD}{}{RESET}",
        yangzz_core::paths::yangzz_dir().display()
    );
    println!();
    println!("  {BOLD}详细文件：{RESET}");
    for (label, path, exists) in yangzz_core::paths::all_paths_report() {
        let mark = if exists {
            format!("{GREEN}✓{RESET}")
        } else {
            format!("{DIM}─{RESET}")
        };
        println!("    {mark} {:<14} {DIM}{}{RESET}", label, path.display());
    }
    println!();
    println!("  {BOLD}完全卸载 yangzz:{RESET}");
    println!("    {GOLD}yangzz --uninstall{RESET}      {DIM}交互式清理 + 卸载提示{RESET}");
    println!();
}

// ── `yangzz --uninstall` ──
// Interactive uninstall: asks whether to wipe user data, then tells the user
// how to remove the binary (we can't reliably self-delete on all platforms).

fn run_uninstall() {
    use std::io::{self, Write};

    fn prompt(msg: &str) -> String {
        print!("  {msg}");
        io::stdout().flush().unwrap();
        let mut buf = String::new();
        if io::stdin().read_line(&mut buf).is_err() {
            return String::new();
        }
        buf.trim().to_string()
    }

    println!();
    println!("  {BOLD}yangzz --uninstall{RESET}");
    println!("  ══════════════════════════");
    println!();

    // Step 1: show what's there
    let report = yangzz_core::paths::all_paths_report();
    let any_exists = report.iter().any(|(_, _, e)| *e);

    if any_exists {
        println!("  {BOLD}检测到以下 yangzz 数据：{RESET}");
        for (label, path, exists) in &report {
            if *exists {
                println!(
                    "    {RED}•{RESET} {:<14} {DIM}{}{RESET}",
                    label,
                    path.display()
                );
            }
        }
        println!();

        let choice = prompt(&format!(
            "{BOLD}要删除这些数据吗？{RESET} {DIM}[y=全删 / n=保留 / Enter=默认保留]{RESET}: "
        ));

        if choice.to_lowercase().starts_with('y') {
            let dir = yangzz_core::paths::yangzz_dir();
            match std::fs::remove_dir_all(&dir) {
                Ok(_) => println!("  {GREEN}✓{RESET} 已删除 {}", dir.display()),
                Err(e) => println!("  {RED}✖{RESET} 删除失败: {e}"),
            }
        } else {
            println!("  {DIM}─ 保留了你的 config / sessions / MEMORY.md{RESET}");
            println!(
                "    {DIM}以后想清：rm -rf {}{RESET}",
                yangzz_core::paths::yangzz_dir().display()
            );
        }
    } else {
        println!("  {DIM}没有找到 yangzz 数据（已经很干净了）。{RESET}");
    }

    // Step 2: tell them how to remove the binary
    println!();
    println!("  {BOLD}最后一步（删二进制）yangzz 自己不能删自己，请选一条：{RESET}");
    println!();
    println!("    {GOLD}npm:{RESET}   npm uninstall -g yangzz");
    println!("    {GOLD}cargo:{RESET} cargo uninstall yangzz");
    println!("    {GOLD}手动:{RESET} rm $(which yangzz)");
    println!();
    println!("  {DIM}再见 👋{RESET}");
    println!();
}
