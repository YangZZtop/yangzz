/// Locale detection + bilingual strings (zh-CN / en)
///
/// Auto-detects from LANG / LC_ALL / LANGUAGE env vars.
/// Usage: `t().welcome_back` or `t().help_title`

use std::sync::OnceLock;

static LOCALE: OnceLock<Lang> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Zh,
    En,
}

pub fn lang() -> Lang {
    *LOCALE.get_or_init(detect_lang)
}

fn detect_lang() -> Lang {
    // Check YANGZZ_LANG first (user override)
    if let Ok(v) = std::env::var("YANGZZ_LANG") {
        return match v.to_lowercase().as_str() {
            "zh" | "cn" | "zh_cn" | "zh-cn" | "chinese" => Lang::Zh,
            _ => Lang::En,
        };
    }
    // Auto-detect from system locale
    for var in &["LANG", "LC_ALL", "LANGUAGE", "LC_MESSAGES"] {
        if let Ok(v) = std::env::var(var) {
            let lo = v.to_lowercase();
            if lo.starts_with("zh") {
                return Lang::Zh;
            }
        }
    }
    Lang::En
}

/// Get the current translation set
pub fn t() -> &'static Strings {
    match lang() {
        Lang::Zh => &ZH,
        Lang::En => &EN,
    }
}

#[allow(dead_code)]
pub struct Strings {
    // ── Banner ──
    pub tagline: &'static str,
    pub welcome_back: &'static str,        // "欢迎回来"
    pub welcome_back_user: &'static str,    // "欢迎回来，{}！" (use with format!)
    pub banner_hint: &'static str,
    #[allow(dead_code)]
    pub input_hint: &'static str,         // "Enter 发送 · Esc 退出"

    // ── Help ──
    pub help_title: &'static str,
    pub help_help: &'static str,
    pub help_model: &'static str,
    pub help_model_name: &'static str,
    pub help_provider: &'static str,
    pub help_tools: &'static str,
    pub help_skills: &'static str,
    pub help_clear: &'static str,
    pub help_status: &'static str,
    pub help_quit: &'static str,
    pub skills_title: &'static str,
    pub env_hint: &'static str,

    // ── Model ──
    pub switch_model: &'static str,
    pub current_label: &'static str,
    pub fetching_models: &'static str,
    pub custom_model: &'static str,
    pub custom_model_prompt: &'static str,
    pub history_kept: &'static str,         // "(对话历史已保留，共{}条…)"

    // ── Commands ──
    pub conversation_cleared: &'static str,
    pub session_saved: &'static str,
    pub tip_prefix: &'static str,
    pub tools_title: &'static str,
    pub current_colon: &'static str,
    pub usage_provider: &'static str,
    pub switched_to: &'static str,

    // ── Status ──
    pub cooked_for: &'static str,           // "⏱ 耗时 {}"
    pub tokens_label: &'static str,

    // ── Setup wizard ──
    pub setup_tagline: &'static str,
    pub setup_welcome: &'static str,
    pub setup_step1: &'static str,
    pub setup_step2: &'static str,
    pub setup_step2_desc: &'static str,
    pub setup_step3: &'static str,
    pub setup_reload: &'static str,
    pub setup_run: &'static str,
    pub setup_advanced: &'static str,
    pub setup_project_config: &'static str,
    pub setup_override_model: &'static str,
    pub setup_switch_repl: &'static str,
    pub setup_rerun: &'static str,
}

const ZH: Strings = Strings {
    tagline: "终端 AI 助手",
    welcome_back: "欢迎回来！",
    welcome_back_user: "欢迎回来，",  // + user + "！"
    banner_hint: "/help 查看命令 · /model 切换模型 · 直接输入开始对话",
    input_hint: "Enter 发送 · /help 帮助 · /quit 退出",

    help_title: "命令列表",
    help_help: "显示帮助",
    help_model: "选择模型（交互式）",
    help_model_name: "直接切换到指定模型",
    help_provider: "切换服务商",
    help_tools: "查看可用工具",
    help_skills: "查看可用技能",
    help_clear: "清空对话历史",
    help_status: "会话统计",
    help_quit: "退出（自动保存）",
    skills_title: "技能",
    env_hint: "环境变量：YANGZZ_MODEL, YANGZZ_BASE_URL, YANGZZ_API_KEY",

    switch_model: "切换模型",
    current_label: "当前",
    fetching_models: "正在获取模型列表…",
    custom_model: "✎ 手动输入模型名…",
    custom_model_prompt: "模型名",
    history_kept: "对话历史已保留，共 {} 条消息 — 新模型无缝接手",

    conversation_cleared: "对话已清空。",
    session_saved: "会话已保存，再见！",
    tip_prefix: "提示：下次可以直接输入",
    tools_title: "工具",
    current_colon: "当前：",
    usage_provider: "用法：/provider <名称>",
    switched_to: "已切换：",

    cooked_for: "⏱ 耗时",
    tokens_label: "token",

    setup_tagline: "终端 AI 助手 — 首次配置",
    setup_welcome: "欢迎！30 秒完成配置。",
    setup_step1: "━━ 第一步：选择服务商 ━━",
    setup_step2: "━━ 第二步：设置环境变量 ━━",
    setup_step2_desc: "复制以下内容到你的 ~/.zshrc 或 ~/.bashrc：",
    setup_step3: "━━ 第三步：重新加载并启动 ━━",
    setup_reload: "# 或者重启终端",
    setup_run: "# 开始对话！",
    setup_advanced: "━━ 进阶 ━━",
    setup_project_config: "项目级配置：在项目根目录创建 .yangzz.toml",
    setup_override_model: "指定模型：yangzz --model gpt-4o-mini",
    setup_switch_repl: "REPL 内切换：/model（交互式选择）",
    setup_rerun: "重新运行此向导：yangzz --setup",
};

/// Translate a tool description for display (API schema stays English)
pub fn translate_tool_desc(name: &str, original: &str) -> String {
    if lang() != Lang::Zh {
        return original.to_string();
    }
    match name {
        "bash" => "在终端执行命令。可运行脚本、安装依赖或任何 Shell 操作。".into(),
        "file_read" => "读取文件内容，支持指定行范围。".into(),
        "file_write" => "创建新文件或覆盖已有文件。".into(),
        "file_edit" => "精确替换文件中的指定内容（old_string 必须完全匹配）。".into(),
        "grep" => "使用 ripgrep/grep 搜索文件中的模式，返回匹配行及文件路径和行号。".into(),
        _ => original.to_string(),
    }
}

/// Translate a skill description for display
pub fn translate_skill_desc(name: &str, original: &str) -> String {
    if lang() != Lang::Zh {
        return original.to_string();
    }
    match name {
        "review" => "代码审查 — 分析 Bug、代码风格和改进建议".into(),
        "debug" => "调试 — 系统化根因分析".into(),
        "explain" => "讲解 — 深入理解代码或概念".into(),
        _ => original.to_string(),
    }
}

const EN: Strings = Strings {
    tagline: "terminal ai assistant",
    welcome_back: "Welcome back!",
    welcome_back_user: "Welcome back, ",  // + user + "!"
    banner_hint: "/help for commands · /model to switch · type to chat",
    input_hint: "Enter to send · /help for help · /quit to exit",

    help_title: "Commands",
    help_help: "Show this help",
    help_model: "Select model (interactive)",
    help_model_name: "Switch to specific model",
    help_provider: "Switch provider",
    help_tools: "List tools",
    help_skills: "List skills",
    help_clear: "Clear conversation",
    help_status: "Session stats",
    help_quit: "Exit (saves session)",
    skills_title: "Skills",
    env_hint: "Environment: YANGZZ_MODEL, YANGZZ_BASE_URL, YANGZZ_API_KEY",

    switch_model: "Switch model",
    current_label: "Current",
    fetching_models: "Fetching models…",
    custom_model: "✎ Enter custom model name…",
    custom_model_prompt: "Model name",
    history_kept: "History preserved ({} messages) — new model continues seamlessly",

    conversation_cleared: "Conversation cleared.",
    session_saved: "Session saved. Bye!",
    tip_prefix: "Tip: type",
    tools_title: "Tools",
    current_colon: "Current:",
    usage_provider: "Usage: /provider <name>",
    switched_to: "Switched to:",

    cooked_for: "⏱ Cooked for",
    tokens_label: "tokens",

    setup_tagline: "terminal ai assistant — first time setup",
    setup_welcome: "Welcome! Let's get you configured in 30 seconds.",
    setup_step1: "━━ Step 1: Choose a provider ━━",
    setup_step2: "━━ Step 2: Set environment variables ━━",
    setup_step2_desc: "Copy-paste one of these into your ~/.zshrc or ~/.bashrc:",
    setup_step3: "━━ Step 3: Reload & run ━━",
    setup_reload: "# or restart terminal",
    setup_run: "# start chatting!",
    setup_advanced: "━━ Advanced ━━",
    setup_project_config: "Per-project config: create .yangzz.toml in project root",
    setup_override_model: "Override model: yangzz --model gpt-4o-mini",
    setup_switch_repl: "Switch in REPL: /model (interactive picker)",
    setup_rerun: "Re-run this: yangzz --setup",
};
