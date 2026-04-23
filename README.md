# yangzz

> AI coding assistant — any model, one variable, ready to go.

**yangzz** 是一个终端原生的 AI 编程助手，用 Rust 编写。支持任意大模型 Provider，一个环境变量即可启动。

## Quick Start

```bash
# 1. 安装
cargo install --path crates/yangzz

# 2. 设置 API Key（任选一个）
export YANGZZ_API_KEY=sk-ant-xxx    # Anthropic（自动识别）
export YANGZZ_API_KEY=sk-xxx        # OpenAI（自动识别）
export ANTHROPIC_API_KEY=sk-ant-xxx  # 也行
export OPENAI_API_KEY=sk-xxx         # 也行

# 3. 运行
yangzz                              # 交互模式
yangzz "explain this codebase"      # 单次执行
```

## Features

- **轻配置** — 一个环境变量启动，Key 前缀自动识别 Provider
- **全 Provider** — 内置 7 个预设：Anthropic, OpenAI, Gemini, DeepSeek, GLM, Grok, Ollama
- **Agentic Loop** — 自动调用工具完成任务（读文件、改代码、跑命令、搜索）
- **Skill 系统** — 内置 review/debug/explain，支持自定义 SKILL.md
- **StatusLine** — 实时显示模型、Git 分支、Token 用量
- **会话持久化** — 自动保存，支持恢复
- **权限管道** — 读操作自动通过，写操作交互确认
- **Rust** — 单二进制，启动快，内存小

## Supported Providers

| Provider | 环境变量 | Key 前缀 | 默认模型 |
|----------|---------|----------|---------|
| Anthropic | `ANTHROPIC_API_KEY` | `sk-ant-` | claude-sonnet-4-20250514 |
| OpenAI | `OPENAI_API_KEY` | `sk-` | gpt-4o |
| Gemini | `GEMINI_API_KEY` | `AIza` | gemini-2.5-pro |
| DeepSeek | `DEEPSEEK_API_KEY` | — | deepseek-chat |
| GLM | `GLM_API_KEY` | — | glm-4-plus |
| Grok | `GROK_API_KEY` | `xai-` | grok-3 |
| Ollama | — | — | llama3 |

或统一使用 `YANGZZ_API_KEY`，自动识别。

## Usage

### 交互模式

```bash
yangzz
```

### 单次执行

```bash
yangzz "fix the bug in src/main.rs"
```

### 指定 Provider / Model

```bash
yangzz --provider deepseek --model deepseek-chat
yangzz --provider ollama --model llama3
yangzz --model gpt-4o "explain this function"
```

### 自定义 API 地址

```bash
yangzz --base-url https://my-proxy.com --model gpt-4o
```

## Commands

| 命令 | 功能 |
|------|------|
| `/help` | 显示帮助 |
| `/quit` | 退出（自动保存会话） |
| `/clear` | 清空对话 |
| `/model` | 显示当前模型 |
| `/tools` | 列出可用工具 |
| `/skills` | 列出可用 Skill |
| `/status` | 显示 StatusLine |

## Built-in Tools

| 工具 | 功能 |
|------|------|
| `bash` | 执行 shell 命令 |
| `file_read` | 读取文件内容 |
| `file_edit` | 精确替换文件内容 |
| `file_write` | 创建或覆写文件 |
| `grep` | 搜索代码（rg/grep） |

## Built-in Skills

| Skill | 触发词 | 功能 |
|-------|--------|------|
| review | `review`, `/review` | 代码审查 |
| debug | `debug`, `/debug` | 系统化调试 |
| explain | `explain`, `/explain` | 代码解释 |

### 自定义 Skill

在项目目录创建 `.yangzz/skills/my-skill.md`：

```markdown
---
name: "my-skill"
description: "My custom skill"
triggers:
- "my-skill"
- "/my-skill"
---

Your skill prompt here...
```

## Configuration

优先级：CLI 参数 > 环境变量 > 厂商环境变量 > 配置文件 > 内置预设

### 环境变量

| 变量 | 说明 |
|------|------|
| `YANGZZ_API_KEY` | API Key（自动识别 Provider） |
| `YANGZZ_PROVIDER` | 指定 Provider |
| `YANGZZ_MODEL` | 指定模型 |
| `YANGZZ_BASE_URL` | 自定义 API 地址 |

### 配置文件

项目级：`.yangzz.toml` 或 `.yangzz/config.toml`
全局级：`~/.config/yangzz/config.toml`

```toml
provider = "anthropic"
model = "claude-sonnet-4-20250514"
max_tokens = 16384
```

## Architecture

```
yangzz-core (库)
├── Provider   — 跟 AI 说话
├── Tool       — 操作环境
├── Loop       — 循环推进
├── Config     — 用户配置
└── Render     — 呈现结果

yangzz (CLI)
└── REPL + 集成层
```

## License

MIT
