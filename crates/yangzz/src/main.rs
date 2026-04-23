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
    println!("    Mac:   ~/Library/Application Support/yangzz/config.toml");
    println!("    Linux: ~/.config/yangzz/config.toml");
    println!("    项目级: .yangzz.toml 或 .yangzz/config.toml");
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
