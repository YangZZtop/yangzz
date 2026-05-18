mod cli_mode;
mod guide;
mod repl;
mod repl_commands;
mod repl_help;
mod repl_render;
mod slash;
#[cfg(feature = "tui")]
mod tui;
mod ui;

use clap::Parser;
use std::sync::Arc;
use yangzz_core::config::settings::CliOverrides;
use yangzz_core::config::{self, Settings};
use yangzz_core::permission::PermissionManager;
use yangzz_core::tool::{ToolExecutor, ToolRegistry};
use yangzz_core::{mcp, plugin};

pub use guide::print_guide;
use ui::format::*;

#[derive(Parser)]
#[command(name = "yangzz")]
#[command(about = "AI coding assistant — any model, one variable, ready to go")]
#[command(version)]
struct Cli {
    /// Initial prompt (if provided, runs in single-shot mode)
    prompt: Option<String>,

    /// Provider name (openai, anthropic, gemini, deepseek, glm, grok, xiaomi, ollama, bedrock, vertex, or custom)
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
    #[cfg(feature = "tui")]
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

#[allow(unused_variables)]
fn cli_wants_tui(cli: &Cli) -> bool {
    #[cfg(feature = "tui")]
    {
        cli.tui
    }
    #[cfg(not(feature = "tui"))]
    {
        false
    }
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

    // Silently migrate legacy data (pre-v0.3.0) on every startup.
    // Best-effort; safe to run repeatedly.
    if yangzz_core::paths::maybe_migrate_legacy() {
        println!(
            "  {DIM}已自动迁移旧版数据 → {}{RESET}",
            yangzz_core::paths::yangzz_dir().display()
        );
    }

    let raw_args: Vec<String> = std::env::args().collect();
    if let Some(detected) = cli_mode::detect_cli_command(&raw_args) {
        let cli = Cli::parse_from(&detected.parse_args);
        let cwd = std::env::current_dir()?;
        let registry = build_tool_registry(&cwd).await;
        let permission = Arc::new(PermissionManager::new());
        let executor = ToolExecutor::new(registry, permission, cwd);
        cli_mode::run_cli_command(
            &detected,
            CliOverrides {
                provider: cli.provider,
                model: cli.model,
                api_key: cli.api_key,
                base_url: cli.base_url,
            },
            &executor,
        )
        .await?;
        return Ok(());
    }

    let cli = Cli::parse();

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

    // Capture TUI flag before cli fields are moved
    let wants_tui = cli_wants_tui(&cli);

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
    let registry = build_tool_registry(&cwd).await;

    if let Some(prompt) = cli.prompt {
        // Single-shot mode — REPL-style rendering to stdout makes sense for
        // pipelines (yangzz "explain" | tee out.txt).
        let permission = Arc::new(PermissionManager::new());
        let executor = ToolExecutor::new(registry, permission, cwd)
            .with_provider(Arc::clone(&provider), model.clone(), max_tokens);
        repl::single_shot(&provider, &model, max_tokens, &prompt, &executor).await?;
    } else if cfg!(feature = "tui") && wants_tui {
        // Opt-in: experimental TUI (alternate-screen + modal permissions).
        // Trade-off: loses native terminal scroll / selection / copy.
        #[cfg(feature = "tui")]
        {
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
        }
    } else {
        // Default (v0.3.1+): classic REPL — the documented product.
        // Native scroll, native selection, full slash-command fidelity.
        // `--repl` is now a no-op but kept so existing scripts don't break.
        let _ = cli.repl;
        let permission = Arc::new(PermissionManager::new());
        let executor = ToolExecutor::new(registry, permission, cwd)
            .with_provider(Arc::clone(&provider), model.clone(), max_tokens);
        repl::run(&provider, &model, max_tokens, &executor, &settings).await?;
    }

    Ok(())
}

async fn build_tool_registry(cwd: &std::path::Path) -> ToolRegistry {
    let mut registry = ToolRegistry::with_builtins(cwd);

    for tool in plugin::load_plugins(cwd) {
        registry.register(tool);
    }

    for tool in mcp::load_mcp_runtime_tools(cwd).await {
        registry.register(tool);
    }

    registry
}

// ── Interactive Setup Wizard ──
// 核心心智模型（基于 cc-switch / sub2api / new-api 三项目分析）：
//   - provider  = 你给这份配置起的名字（不是厂商）
//   - base_url  = 实际入口地址（允许带路径前缀，如 /antigravity）
//   - api_format = 入口用什么协议说话（openai / anthropic / gemini / vertex / bedrock）
//   - model     = 希望上游路由到的模型名
// 只问必要问题，其他给合理默认。

fn normalize_setup_api_format(raw: &str) -> Option<String> {
    let normalized = raw.trim().to_lowercase();
    match normalized.as_str() {
        "" => Some("openai".to_string()),
        "openai" | "anthropic" | "gemini" | "vertex" | "bedrock" => Some(normalized),
        _ => None,
    }
}

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

    println!();
    println!("  {BOLD_GOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{RESET}");
    println!("  {BOLD}  欢迎使用 yangzz！{RESET}");
    println!("  {DIM}  终端 AI 编程助手 — 3 步配置，马上能用{RESET}");
    println!("  {BOLD_GOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{RESET}");
    println!();

    // ── Step 0: 场景选择 ──
    println!("  {BOLD}你的 AI 接口来自哪里？{RESET}");
    println!();
    println!("    {GOLD}1{RESET}  中转站（最常见：从中转商买的地址+Key）");
    println!("    {GOLD}2{RESET}  本地 Ollama（免费离线，不需要 Key）");
    println!("    {GOLD}3{RESET}  官方 API（直连 OpenAI / Anthropic / Google）");
    println!();
    let choice = prompt_default("选一个", "1");

    match choice.as_str() {
        "2" => return setup_ollama(),
        "3" => return setup_official(),
        _ => {} // 继续中转站流程
    }

    // ── 中转站流程（最常见） ──
    println!();
    println!("  {BOLD}好的，配置你的中转站：{RESET}");
    println!("  {DIM}（从中转商那里拿到的地址和 Key 填进来就行）{RESET}");
    println!();

    // Step 1: 地址
    let url = prompt("  {BOLD}中转地址: {RESET}");
    if url.is_empty() {
        println!("  {RED}✖{RESET} 地址不能为空");
        return false;
    }

    // Step 2: Key
    let key = prompt("  {BOLD}API Key: {RESET}");

    // Step 3: 自动获取模型，让用户选
    println!();
    print!("  {DIM}正在获取可用模型…{RESET}");
    let _ = io::stdout().flush();

    let api_format = infer_api_format_from_url(&url, "");
    let name = infer_provider_name(&url);

    // Try to fetch models from the provider
    let fetched_models = std::thread::spawn({
        let url = url.clone();
        let key = key.clone();
        let api_format = api_format.clone();
        move || {
            let rt = tokio::runtime::Runtime::new().ok()?;
            rt.block_on(async {
                let tmp_settings = yangzz_core::config::Settings {
                    provider: Some("setup".into()),
                    api_key: Some(key),
                    base_url: Some(url),
                    api_format: Some(api_format),
                    providers: vec![],
                    ..Default::default()
                };
                let provider = yangzz_core::config::resolve_provider(&tmp_settings).ok()?;
                tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    provider.list_models(),
                )
                .await
                .ok()?
                .ok()
            })
        }
    })
    .join()
    .ok()
    .flatten()
    .unwrap_or_default();

    print!("\r\x1b[2K"); // Clear "正在获取…"
    let _ = io::stdout().flush();

    let model = if !fetched_models.is_empty() {
        println!("  {GREEN}✓{RESET} 找到 {BOLD}{}{RESET} 个可用模型：", fetched_models.len());
        println!();
        for (i, m) in fetched_models.iter().enumerate().take(15) {
            println!("    {GOLD}{:>2}{RESET}  {m}", i + 1);
        }
        if fetched_models.len() > 15 {
            println!("    {DIM}… 还有 {} 个{RESET}", fetched_models.len() - 15);
        }
        println!();
        let choice = prompt_default("选一个（输入序号或模型名）", "1");
        // Parse as number or use as model name directly
        if let Ok(idx) = choice.parse::<usize>() {
            if idx >= 1 && idx <= fetched_models.len() {
                fetched_models[idx - 1].clone()
            } else {
                choice
            }
        } else {
            choice
        }
    } else {
        println!("  {DIM}(未能获取模型列表，请手动输入){RESET}");
        println!("  {DIM}常用：claude-sonnet-4-20250514 / gpt-4o / deepseek-chat{RESET}");
        prompt_default("默认模型", "claude-sonnet-4-20250514")
    };

    // ── 自动推断 api_format ──
    let api_format = infer_api_format_from_url(&url, &model);

    // ── 自动生成配置名 ──
    let name = infer_provider_name(&url);

    // ── Write config ──
    yangzz_core::paths::ensure_yangzz_dir();
    let config_path = yangzz_core::paths::config_path();

    if config_path.exists() {
        let overwrite = prompt_default(&format!("  {YELLOW}⚠{RESET} 配置已存在，覆盖？(y/n)"), "n");
        if !overwrite.to_lowercase().starts_with('y') {
            println!("  已取消。");
            return false;
        }
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
    println!("  {GREEN}✓ 配置完成！{RESET}");
    println!("  {DIM}文件: {}{RESET}", config_path.display());
    println!();
    println!("  {BOLD}现在可以：{RESET}");
    println!("    • 直接打字提问，回车发送");
    println!("    • {GOLD}/model{RESET}    切换模型");
    println!("    • {GOLD}/thinking{RESET} 调整思考深度");
    println!("    • {GOLD}/help{RESET}     查看所有命令");
    println!("    • {GOLD}Ctrl+C{RESET}    取消 / {GOLD}Ctrl+D{RESET} 退出");
    println!();

    true
}

/// Ollama 本地配置（零门槛）
fn setup_ollama() -> bool {
    use std::io::{self, Write};

    println!();
    println!("  {BOLD}Ollama 本地模式{RESET}");
    println!("  {DIM}确保 Ollama 已启动（ollama serve）{RESET}");
    println!();

    print!("  模型名 {DIM}[llama3]{RESET}: ");
    io::stdout().flush().unwrap();
    let mut model = String::new();
    let _ = io::stdin().read_line(&mut model);
    let model = model.trim();
    let model = if model.is_empty() { "llama3" } else { model };

    yangzz_core::paths::ensure_yangzz_dir();
    let config_path = yangzz_core::paths::config_path();
    let toml = format!(
        r#"provider = "ollama"
model = "{model}"

[[providers]]
name = "ollama"
api_key = ""
base_url = "http://localhost:11434"
default_model = "{model}"
api_format = "openai"
"#
    );

    if let Err(e) = std::fs::write(&config_path, &toml) {
        println!("  {RED}✖{RESET} 写入失败: {e}");
        return false;
    }

    println!();
    println!("  {GREEN}✓ 配置完成！{RESET} 使用 Ollama 本地模型 {BOLD}{model}{RESET}");
    println!();
    true
}

/// 官方 API 直连配置
fn setup_official() -> bool {
    use std::io::{self, Write};

    println!();
    println!("  {BOLD}选择官方 API：{RESET}");
    println!("    {GOLD}1{RESET}  OpenAI（GPT-4o / GPT-5 / o3）");
    println!("    {GOLD}2{RESET}  Anthropic（Claude Sonnet / Opus）");
    println!("    {GOLD}3{RESET}  Google Gemini");
    println!();

    print!("  选择 {DIM}[1]{RESET}: ");
    io::stdout().flush().unwrap();
    let mut choice = String::new();
    let _ = io::stdin().read_line(&mut choice);
    let choice = choice.trim();

    let (name, base_url, api_format, default_model, key_hint) = match choice {
        "2" => ("anthropic", "https://api.anthropic.com", "anthropic", "claude-sonnet-4-20250514", "sk-ant-..."),
        "3" => ("gemini", "https://generativelanguage.googleapis.com", "gemini", "gemini-2.5-pro", "AIza..."),
        _ => ("openai", "https://api.openai.com", "openai", "gpt-4o", "sk-..."),
    };

    println!();
    print!("  API Key ({DIM}{key_hint}{RESET}): ");
    io::stdout().flush().unwrap();
    let mut key = String::new();
    let _ = io::stdin().read_line(&mut key);
    let key = key.trim();

    if key.is_empty() {
        println!("  {RED}✖{RESET} 官方 API 需要 Key");
        return false;
    }

    yangzz_core::paths::ensure_yangzz_dir();
    let config_path = yangzz_core::paths::config_path();
    let toml = format!(
        r#"provider = "{name}"
model = "{default_model}"

[[providers]]
name = "{name}"
api_key = "{key}"
base_url = "{base_url}"
default_model = "{default_model}"
api_format = "{api_format}"
"#
    );

    if let Err(e) = std::fs::write(&config_path, &toml) {
        println!("  {RED}✖{RESET} 写入失败: {e}");
        return false;
    }

    println!();
    println!("  {GREEN}✓ 配置完成！{RESET} 使用 {BOLD}{name}{RESET} · {default_model}");
    println!();
    true
}

/// 从 URL 推断 api_format
fn infer_api_format_from_url(url: &str, model: &str) -> String {
    let lower_url = url.to_lowercase();
    let lower_model = model.to_lowercase();

    // 官方域名直接判断
    if lower_url.contains("anthropic.com") {
        return "anthropic".to_string();
    }
    if lower_url.contains("generativelanguage.googleapis.com") {
        return "gemini".to_string();
    }

    // 如果模型名是 Claude 且 URL 不是已知的 OpenAI 兼容中转，可能是 Anthropic 格式
    // 但绝大多数中转都是 OpenAI 格式，即使调 Claude 也是
    // 所以默认 openai，除非 URL 明确是 Anthropic
    if lower_url.contains("anyrouter") || lower_url.contains("packycode") {
        return "anthropic".to_string();
    }

    // 默认 OpenAI 兼容（覆盖 99% 的中转站）
    "openai".to_string()
}

/// 从 URL 生成一个简短的 provider 名
fn infer_provider_name(url: &str) -> String {
    // 尝试提取域名中有意义的部分
    let cleaned = url
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or("my-relay");

    // 如果是 IP 地址，用 "my-relay"
    if cleaned.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(true) {
        return "my-relay".to_string();
    }

    // 取域名第一段
    let name = cleaned.split('.').next().unwrap_or("my-relay");
    if name.len() > 20 {
        "my-relay".to_string()
    } else {
        name.to_string()
    }
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

#[cfg(test)]
mod tests {
    use super::normalize_setup_api_format;

    #[test]
    fn normalize_setup_api_format_supports_documented_drivers() {
        assert_eq!(normalize_setup_api_format("").as_deref(), Some("openai"));
        assert_eq!(
            normalize_setup_api_format("openai").as_deref(),
            Some("openai")
        );
        assert_eq!(
            normalize_setup_api_format("anthropic").as_deref(),
            Some("anthropic")
        );
        assert_eq!(
            normalize_setup_api_format("gemini").as_deref(),
            Some("gemini")
        );
        assert_eq!(
            normalize_setup_api_format("vertex").as_deref(),
            Some("vertex")
        );
        assert_eq!(
            normalize_setup_api_format("bedrock").as_deref(),
            Some("bedrock")
        );
        assert_eq!(normalize_setup_api_format("custom"), None);
    }
}
