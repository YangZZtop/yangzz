mod repl;
mod tui;
mod ui;

use clap::Parser;
use std::sync::Arc;
use yangzz_core::config::{self, Settings};
use yangzz_core::config::settings::CliOverrides;
use yangzz_core::permission::PermissionManager;
use yangzz_core::tool::{ToolExecutor, ToolRegistry};

use ui::format::*;

#[derive(Parser)]
#[command(name = "yangzz")]
#[command(about = "AI coding assistant — any model, one variable, ready to go")]
#[command(version)]
struct Cli {
    /// Initial prompt (if provided, runs in single-shot mode)
    prompt: Option<String>,

    /// Provider name (anthropic, openai, gemini, deepseek, glm, grok, ollama)
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

    /// Use TUI mode (dual-pane terminal UI)
    #[arg(long)]
    tui: bool,

    /// Show quick-start guide
    #[arg(long)]
    guide: bool,

    /// Run health check (config, provider, tools)
    #[arg(long)]
    doctor: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Init logging — only show on RUST_LOG, silent by default
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("yangzz=warn".parse()?)
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

    // --guide flag: show quick-start guide
    if cli.guide {
        print_guide();
        return Ok(());
    }

    // --setup flag: always run wizard
    if cli.setup {
        run_setup_wizard();
        return Ok(());
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
            run_setup_wizard();
            std::process::exit(0);
        }
    };

    let model = settings.resolved_model();
    let max_tokens = settings.resolved_max_tokens();

    // Setup tool system
    let cwd = std::env::current_dir()?;
    let registry = ToolRegistry::with_builtins(&cwd);

    if let Some(prompt) = cli.prompt {
        let permission = Arc::new(PermissionManager::new());
        let executor = ToolExecutor::new(registry, permission, cwd);
        repl::single_shot(&provider, &model, max_tokens, &prompt, &executor).await?;
    } else if cli.tui {
        // TUI mode: auto-approve tools (raw mode breaks stdin prompts)
        let permission = Arc::new(PermissionManager::auto_approve());
        let executor = ToolExecutor::new(registry, permission, cwd);
        tui::run(&provider, &model, max_tokens, Arc::new(executor), &settings).await?;
    } else {
        let permission = Arc::new(PermissionManager::new());
        let executor = ToolExecutor::new(registry, permission, cwd);
        repl::run(&provider, &model, max_tokens, &executor, &settings).await?;
    }

    Ok(())
}

// ── Quick-start Guide (yangzz --guide) ──

pub fn print_guide() {
    println!();
    println!("  {BOLD}yangzz 快速上手指南{RESET}");
    println!("  ══════════════════");
    println!();
    println!("  {BOLD_GOLD}一、配置（只需做一次）{RESET}");
    println!();
    println!("  yangzz 的配置写在自己的文件里，不影响其他工具。");
    println!();
    println!("  {BOLD}配置文件位置：{RESET}");
    println!("    Mac:     ~/Library/Application Support/yangzz/config.toml");
    println!("    Linux:   ~/.config/yangzz/config.toml");
    println!("    Windows: %APPDATA%\\yangzz\\config.toml");
    println!("    项目级:   .yangzz.toml 或 .yangzz/config.toml");
    println!();
    println!("  {BOLD_GOLD}最常见：用中转站{RESET}");
    println!();
    println!("  从中转商拿到 地址+Key 后，创建配置文件写入：");
    println!();
    println!("    {DIM}# ~/Library/Application Support/yangzz/config.toml{RESET}");
    println!();
    println!("    {BOLD}provider = \"my-relay\"{RESET}");
    println!("    {BOLD}model = \"claude-sonnet-4-20250514\"{RESET}");
    println!();
    println!("    {BOLD}[[providers]]{RESET}");
    println!("    {BOLD}name = \"my-relay\"{RESET}");
    println!("    {BOLD}api_key = \"sk-你的key\"{RESET}");
    println!("    {BOLD}base_url = \"https://你的中转地址\"{RESET}");
    println!("    {BOLD}default_model = \"claude-sonnet-4-20250514\"{RESET}");
    println!("    {BOLD}api_format = \"openai\"{RESET}          {DIM}# 绝大多数中转都是 openai 格式{RESET}");
    println!("    {BOLD}max_tokens = 16384{RESET}             {DIM}# 单次最大输出（可选）{RESET}");
    println!("    {BOLD}thinking_budget = 10000{RESET}        {DIM}# 思考深度（可选）{RESET}");
    println!("    {BOLD}context_window = 200000{RESET}        {DIM}# 上下文窗口（可选）{RESET}");
    println!("    {BOLD}reasoning_effort = \"medium\"{RESET}    {DIM}# 推理强度 low/medium/high（可选）{RESET}");
    println!("    {BOLD}temperature = 0.7{RESET}              {DIM}# 创造性 0~1（可选）{RESET}");
    println!();
    println!("  配完直接运行 {BOLD}yangzz{RESET} 即可。");
    println!();
    println!("  {BOLD_GOLD}配多个中转？{RESET}  写多个 [[providers]]，对话中 /model 切换。");
    println!("  {BOLD_GOLD}用 Ollama？{RESET}   base_url = \"http://localhost:11434\"，api_key 随便填。");
    println!();
    println!("  ──────────────────");
    println!();
    println!("  {BOLD_GOLD}二、REPL 常用命令{RESET}");
    println!();
    println!("    {GOLD}/help{RESET}            所有命令");
    println!("    {GOLD}/model <name>{RESET}    切换模型");
    println!("    {GOLD}/undo{RESET}            撤销上次文件编辑");
    println!("    {GOLD}/memory [text]{RESET}   查看/添加记忆");
    println!("    {GOLD}/compact{RESET}         压缩对话历史");
    println!("    {GOLD}/recall <kw>{RESET}     搜索过去会话");
    println!("    {GOLD}/task [cmd]{RESET}      任务队列");
    println!("    {GOLD}/route <text>{RESET}    预览智能路由");
    println!("    {GOLD}/profile{RESET}         项目技术栈检测");
    println!("    {GOLD}/policy{RESET}          查看执行策略");
    println!("    {GOLD}/guide{RESET}           查看本指南");
    println!("    {GOLD}/status{RESET}          Token 用量 + 费用");
    println!("    {GOLD}/quit{RESET}            退出");
    println!();
    println!("  ──────────────────");
    println!();
    println!("  {BOLD_GOLD}三、自动运行的功能（你不需要做任何事）{RESET}");
    println!();
    println!("    • {BOLD}Hermes 自进化{RESET}     — 自动学习你的偏好，写入 MEMORY.md");
    println!("    • {BOLD}自动记忆捕获{RESET}      — 偏好/教训/事实/成功模式自动提取");
    println!("    • {BOLD}完成度检查{RESET}        — 防止 AI 虚报完成，自动追问");
    println!("    • {BOLD}工具失败重试{RESET}      — 超时/连接错误自动重试一次");
    println!("    • {BOLD}项目技能检测{RESET}      — 首轮对话自动识别语言/框架/包管理器");
    println!("    • {BOLD}4层记忆降级{RESET}       — context 快满时自动切换 L0→L1→L2→L3");
    println!("    • {BOLD}危险命令拦截{RESET}      — rm -rf / DROP TABLE 等 27 种模式");
    println!("    • {BOLD}符号链接防护{RESET}      — 自动拒绝写入符号链接");
    println!("    • {BOLD}密钥扫描{RESET}          — 代码中出现 API key 自动警告");
    println!("    • {BOLD}沙箱隔离{RESET}          — 配置 policy.toml 后自动生效");
    println!("    • {BOLD}Auto Compact{RESET}      — 上下文超 75% 自动压缩");
    println!("    • {BOLD}JSON 修复{RESET}         — 弱模型的 JSON 输出自动修复");
    println!("    • {BOLD}Pangu 排版{RESET}        — 中英文之间自动加空格");
    println!();
    println!("  ──────────────────");
    println!();
    println!("  {BOLD_GOLD}四、快捷键{RESET}");
    println!();
    println!("    {GOLD}↑/↓{RESET}       翻阅历史输入");
    println!("    {GOLD}Ctrl+C{RESET}    取消输入或中断 AI");
    println!("    {GOLD}Ctrl+D{RESET}    退出");
    println!("    {GOLD}行尾 \\{RESET}    多行输入");
    println!();
    println!("  {DIM}配置向导: yangzz --setup{RESET}");
    println!();
}

// ── First-time Setup Wizard ──

fn run_setup_wizard() {
    println!();
    println!("  {BOLD_GOLD} █████ ████  ██████   ████████    ███████  █████████  █████████{RESET}");
    println!("  {BOLD_GOLD} ░░███ ░███  ░░░░░███ ░░███░░███  ███░░███ ░█░░░░███  ░█░░░░███{RESET}");
    println!("  {BOLD_GOLD}  ░███ ░███   ███████  ░███ ░███ ░███ ░███ ░   ███░   ░   ███░{RESET}");
    println!("  {BOLD_GOLD}  ░███ ░███  ███░░███  ░███ ░███ ░███ ░███   ███░   █   ███░   █{RESET}");
    println!("  {BOLD_GOLD}  ░░███████ ░░████████ ████ █████░░███████  █████████  █████████{RESET}");
    println!("  {BOLD_GOLD}   ░░░░░███  ░░░░░░░░ ░░░░ ░░░░░  ░░░░░███ ░░░░░░░░░  ░░░░░░░░░{RESET}");
    println!("  {BOLD_GOLD}   ███ ░███                       ███ ░███{RESET}");
    println!("  {BOLD_GOLD}  ░░██████                       ░░██████{RESET}");
    println!("  {BOLD_GOLD}   ░░░░░░                         ░░░░░░{RESET}");
    println!();
    println!("  {BOLD}欢迎使用 yangzz！{RESET} {DIM}AI coding assistant — 多模型、多中转、开箱即用{RESET}");
    println!();
    println!("  ──────────────────");
    println!();
    println!("  {BOLD_GOLD}第一步：创建配置文件{RESET}");
    println!();

    // Detect config path
    let config_path = if cfg!(target_os = "macos") {
        "~/Library/Application Support/yangzz/config.toml".to_string()
    } else if cfg!(target_os = "windows") {
        "%APPDATA%\\yangzz\\config.toml".to_string()
    } else {
        "~/.config/yangzz/config.toml".to_string()
    };
    println!("  在以下路径创建配置文件：");
    println!("    {BOLD}{config_path}{RESET}");
    println!();

    println!("  {BOLD_GOLD}第二步：写入配置（复制粘贴即可）{RESET}");
    println!();
    println!("  {DIM}# ── 用中转站（最常见）──{RESET}");
    println!();
    println!("    {BOLD}provider = \"my-relay\"{RESET}");
    println!("    {BOLD}model = \"claude-sonnet-4-20250514\"{RESET}");
    println!();
    println!("    {BOLD}[[providers]]{RESET}");
    println!("    {BOLD}name = \"my-relay\"{RESET}              {DIM}# 随便起个名字{RESET}");
    println!("    {BOLD}api_key = \"sk-你的key\"{RESET}         {DIM}# 中转商给你的 key{RESET}");
    println!("    {BOLD}base_url = \"https://你的中转地址\"{RESET}  {DIM}# 中转商给你的地址{RESET}");
    println!("    {BOLD}default_model = \"claude-sonnet-4-20250514\"{RESET}");
    println!("    {BOLD}api_format = \"openai\"{RESET}          {DIM}# 绝大多数中转都是 openai 格式{RESET}");
    println!();

    println!("  {DIM}# ── 更多可选配置 ──{RESET}");
    println!("    {DIM}max_tokens = 16384{RESET}             {DIM}# 单次最大输出{RESET}");
    println!("    {DIM}thinking_budget = 10000{RESET}        {DIM}# 思考 token 预算{RESET}");
    println!("    {DIM}context_window = 200000{RESET}        {DIM}# 上下文窗口{RESET}");
    println!("    {DIM}reasoning_effort = \"medium\"{RESET}    {DIM}# low / medium / high{RESET}");
    println!("    {DIM}temperature = 0.7{RESET}              {DIM}# 创造性 0~1{RESET}");
    println!();

    println!("  {DIM}# ── 用本地 Ollama（免费、离线）──{RESET}");
    println!();
    println!("    {DIM}[[providers]]{RESET}");
    println!("    {DIM}name = \"local\"{RESET}");
    println!("    {DIM}api_key = \"ollama\"{RESET}");
    println!("    {DIM}base_url = \"http://localhost:11434\"{RESET}");
    println!("    {DIM}default_model = \"llama3\"{RESET}");
    println!();

    println!("  ──────────────────");
    println!();
    println!("  {BOLD_GOLD}第三步：启动{RESET}");
    println!();
    println!("    {BOLD}yangzz{RESET}");
    println!();

    println!("  {DIM}提示：{RESET}");
    println!("    {DIM}• 可以写多个 [[providers]]，对话中 /model 切换{RESET}");
    println!("    {DIM}• 项目级配置: 项目根目录 .yangzz.toml 覆盖全局{RESET}");
    println!("    {DIM}• 完整指南: yangzz --guide{RESET}");
    println!();
}

// ── Health Check (yangzz --doctor) ──

fn run_doctor() {
    println!();
    println!("  {BOLD}yangzz --doctor{RESET}  Health Check");
    println!("  ══════════════════════════");
    println!();

    let mut passed = 0u32;
    let mut warned = 0u32;
    let mut failed = 0u32;

    // 1. Config file
    let settings = Settings::load(CliOverrides::default());
    let config_path = if cfg!(target_os = "macos") {
        dirs::data_dir().map(|d| d.join("yangzz").join("config.toml"))
    } else if cfg!(target_os = "windows") {
        dirs::config_dir().map(|d| d.join("yangzz").join("config.toml"))
    } else {
        dirs::config_dir().map(|d| d.join("yangzz").join("config.toml"))
    };

    if let Some(ref path) = config_path {
        if path.exists() {
            println!("  {GREEN}✓{RESET} Config file: {}", path.display());
            passed += 1;
        } else {
            // Check project-local
            let cwd = std::env::current_dir().unwrap_or_default();
            let local = cwd.join(".yangzz.toml");
            if local.exists() {
                println!("  {GREEN}✓{RESET} Config file: {} (project-local)", local.display());
                passed += 1;
            } else {
                println!("  {RED}✖{RESET} Config file not found: {}", path.display());
                println!("    {DIM}Run: yangzz --setup{RESET}");
                failed += 1;
            }
        }
    } else {
        println!("  {YELLOW}⚠{RESET} Cannot determine config directory");
        warned += 1;
    }

    // 2. Provider
    if settings.provider.is_some() {
        match config::resolve_provider(&settings) {
            Ok(p) => {
                println!("  {GREEN}✓{RESET} Provider: {} ({})", p.name(), settings.resolved_model());
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
        let has_key = settings.api_key.is_some()
            || settings.providers.iter().any(|p| !p.api_key.is_empty());
        if has_key {
            println!("  {GREEN}✓{RESET} API key configured");
            passed += 1;
        } else {
            println!("  {RED}✖{RESET} No API key found");
            failed += 1;
        }
    } else {
        // Check env vars
        let env_keys = ["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "GEMINI_API_KEY", "DEEPSEEK_API_KEY"];
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
    let provider_count = settings.providers.len();
    if provider_count > 0 {
        println!("  {GREEN}✓{RESET} Extra providers: {provider_count} configured");
        for ep in &settings.providers {
            let model = ep.default_model.as_deref().unwrap_or("(default)");
            println!("    {DIM}• {} → {} [{}]{RESET}", ep.name, ep.base_url, model);
        }
        passed += 1;
    } else {
        println!("  {DIM}─{RESET} No extra [[providers]] (optional)");
    }

    // 5. Rust toolchain
    let has_cargo = std::process::Command::new("cargo").arg("--version").output().is_ok();
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
    if has_git { println!("    {GREEN}✓{RESET} Git repo detected"); passed += 1; }
    if has_memory { println!("    {GREEN}✓{RESET} MEMORY.md found"); passed += 1; }

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
        println!("  {GREEN}■{RESET} {BOLD}All checks passed{RESET} ({passed} ok, {warned} warnings)");
    } else {
        println!("  {RED}■{RESET} {BOLD}{failed} issue(s) found{RESET} ({passed} ok, {warned} warnings)");
        println!("    {DIM}Fix the issues above, then run 'yangzz --doctor' again{RESET}");
    }
    println!();
}
