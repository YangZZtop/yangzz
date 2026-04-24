use crate::emitln;
use crate::ui::format::*;

pub fn print_guide() {
    emitln!();
    emitln!("  {BOLD}yangzz 快速上手指南{RESET}");
    emitln!("  ══════════════════");
    emitln!();
    emitln!("  {BOLD_GOLD}一、配置（只需做一次）{RESET}");
    emitln!();
    emitln!("  yangzz 的配置写在自己的文件里，不影响其他工具。");
    emitln!();
    emitln!("  {BOLD}配置文件位置：{RESET}");
    emitln!("    Mac:     ~/Library/Application Support/yangzz/config.toml");
    emitln!("    Linux:   ~/.config/yangzz/config.toml");
    emitln!("    Windows: %APPDATA%\\yangzz\\config.toml");
    emitln!("    项目级:   .yangzz.toml 或 .yangzz/config.toml");
    emitln!();
    emitln!("  {BOLD_GOLD}最常见：用中转站{RESET}");
    emitln!();
    emitln!("  从中转商拿到 地址+Key 后，创建配置文件写入：");
    emitln!();
    emitln!("    {DIM}# ~/Library/Application Support/yangzz/config.toml{RESET}");
    emitln!();
    emitln!(
        "    {BOLD}provider = \"my-relay\"{RESET}              {DIM}# 配置名，和 [[providers]].name 对应（不是厂商）{RESET}"
    );
    emitln!("    {BOLD}model = \"claude-sonnet-4-20250514\"{RESET}");
    emitln!();
    emitln!("    {BOLD}[[providers]]{RESET}");
    emitln!("    {BOLD}name = \"my-relay\"{RESET}");
    emitln!("    {BOLD}api_key = \"sk-你的key\"{RESET}");
    emitln!(
        "    {BOLD}base_url = \"https://你的中转地址\"{RESET}     {DIM}# 支持带路径前缀，如 .../antigravity{RESET}"
    );
    emitln!("    {BOLD}default_model = \"claude-sonnet-4-20250514\"{RESET}");
    emitln!(
        "    {BOLD}api_format = \"openai\"{RESET}          {DIM}# 入口协议：openai / anthropic / gemini / auto{RESET}"
    );
    emitln!("    {BOLD}max_tokens = 16384{RESET}             {DIM}# 单次最大输出（可选）{RESET}");
    emitln!("    {BOLD}thinking_budget = 32000{RESET}        {DIM}# 思考深度（可选）{RESET}");
    emitln!(
        "    {BOLD}context_window = 1000000{RESET}       {DIM}# 上下文窗口（可选，主流模型已 1M）{RESET}"
    );
    emitln!(
        "    {BOLD}reasoning_effort = \"medium\"{RESET}    {DIM}# 推理强度 low/medium/high（可选）{RESET}"
    );
    emitln!("    {BOLD}temperature = 0.7{RESET}              {DIM}# 创造性 0~1（可选）{RESET}");
    emitln!();
    emitln!("  配完直接运行 {BOLD}yangzz{RESET} 即可。");
    emitln!();
    emitln!("  {BOLD_GOLD}配多个中转？{RESET}  写多个 [[providers]]，对话中 /model 切换。");
    emitln!(
        "  {BOLD_GOLD}用 Ollama？{RESET}   base_url = \"http://localhost:11434\"，api_key 随便填。"
    );
    emitln!();
    emitln!("  ──────────────────");
    emitln!();
    emitln!("  {BOLD_GOLD}二、REPL 常用命令{RESET}");
    emitln!();
    emitln!("    {GOLD}/help{RESET}            所有命令");
    emitln!("    {GOLD}/model <name>{RESET}    切换模型");
    emitln!("    {GOLD}/undo{RESET}            撤销上次文件编辑");
    emitln!("    {GOLD}/memory [text]{RESET}   查看/添加记忆");
    emitln!("    {GOLD}/compact{RESET}         压缩对话历史");
    emitln!("    {GOLD}/recall <kw>{RESET}     搜索过去会话");
    emitln!("    {GOLD}/task [cmd]{RESET}      任务队列");
    emitln!("    {GOLD}/route <text>{RESET}    预览智能路由");
    emitln!("    {GOLD}/profile{RESET}         项目技术栈检测");
    emitln!("    {GOLD}/policy{RESET}          查看执行策略");
    emitln!("    {GOLD}/guide{RESET}           查看本指南");
    emitln!("    {GOLD}/status{RESET}          Token 用量 + 费用");
    emitln!("    {GOLD}/quit{RESET}            退出");
    emitln!();
    emitln!("  ──────────────────");
    emitln!();
    emitln!("  {BOLD_GOLD}三、自动运行的功能（你不需要做任何事）{RESET}");
    emitln!();
    emitln!("    • {BOLD}Hermes 自进化{RESET}     — 自动学习你的偏好，写入 MEMORY.md");
    emitln!("    • {BOLD}自动记忆捕获{RESET}      — 偏好/教训/事实/成功模式自动提取");
    emitln!("    • {BOLD}完成度检查{RESET}        — 防止 AI 虚报完成，自动追问");
    emitln!("    • {BOLD}工具失败重试{RESET}      — 超时/连接错误自动重试一次");
    emitln!("    • {BOLD}项目技能检测{RESET}      — 首轮对话自动识别语言/框架/包管理器");
    emitln!("    • {BOLD}4层记忆降级{RESET}       — context 快满时自动切换 L0→L1→L2→L3");
    emitln!("    • {BOLD}危险命令拦截{RESET}      — rm -rf / DROP TABLE 等 27 种模式");
    emitln!("    • {BOLD}符号链接防护{RESET}      — 自动拒绝写入符号链接");
    emitln!("    • {BOLD}密钥扫描{RESET}          — 代码中出现 API key 自动警告");
    emitln!("    • {BOLD}沙箱隔离{RESET}          — 配置 policy.toml 后自动生效");
    emitln!("    • {BOLD}Auto Compact{RESET}      — 上下文超 75% 自动压缩");
    emitln!("    • {BOLD}JSON 修复{RESET}         — 弱模型的 JSON 输出自动修复");
    emitln!("    • {BOLD}Pangu 排版{RESET}        — 中英文之间自动加空格");
    emitln!();
    emitln!("  ──────────────────");
    emitln!();
    emitln!("  {BOLD_GOLD}四、快捷键{RESET}");
    emitln!();
    emitln!("    {GOLD}↑/↓{RESET}       翻阅历史输入");
    emitln!("    {GOLD}Ctrl+C{RESET}    取消输入或中断 AI");
    emitln!("    {GOLD}Ctrl+D{RESET}    退出");
    emitln!("    {GOLD}行尾 \\{RESET}    多行输入");
    emitln!();
    emitln!("  {DIM}配置向导: yangzz --setup{RESET}");
    emitln!("  {DIM}健康检查: yangzz --doctor{RESET}");
    emitln!();
}
