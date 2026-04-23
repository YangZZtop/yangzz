<div align="center">

```
 █████ ████  ██████   ████████    ███████  █████████  █████████
 ░░███ ░███  ░░░░░███ ░░███░░███  ███░░███ ░█░░░░███  ░█░░░░███
  ░███ ░███   ███████  ░███ ░███ ░███ ░███ ░   ███░   ░   ███░
  ░███ ░███  ███░░███  ░███ ░███ ░███ ░███   ███░   █   ███░   █
  ░░███████ ░░████████ ████ █████░░███████  █████████  █████████
   ░░░░░███  ░░░░░░░░ ░░░░ ░░░░░  ░░░░░███ ░░░░░░░░░  ░░░░░░░░░
   ███ ░███                       ███ ░███
  ░░██████                       ░░██████
   ░░░░░░                         ░░░░░░
```

**终端 AI 编程助手 — 多模型、多中转、开箱即用**

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/built_with-Rust-orange.svg)](https://www.rust-lang.org/)
[![npm](https://img.shields.io/npm/v/yangzz.svg)](https://www.npmjs.com/package/yangzz)

[快速开始](#-快速开始) · [配置指南](#-配置) · [功能特性](#-功能特性) · [命令列表](#-repl-命令) · [架构](#-架构)

</div>

---

## 🚀 快速开始

### 安装

```bash
# npm（推荐）
npm install -g yangzz

# 或 Cargo
cargo install --path crates/yangzz
```

### 配置

创建配置文件（**API Key 只存在这个文件里，不影响其他工具**）：

- **Mac**: `~/Library/Application Support/yangzz/config.toml`
- **Linux**: `~/.config/yangzz/config.toml`
- **Windows**: `%APPDATA%\yangzz\config.toml`

```toml
provider = "my-relay"
model = "claude-sonnet-4-20250514"

[[providers]]
name = "my-relay"                      # 随便起个名字
api_key = "sk-你的key"                  # 中转商给你的 key
base_url = "https://你的中转地址"        # 中转商给你的地址
default_model = "claude-sonnet-4-20250514"
api_format = "openai"                  # 绝大多数中转都是 openai 格式
```

### 启动

```bash
yangzz                                 # 交互模式
yangzz "fix the bug in src/main.rs"    # 单次执行
yangzz --guide                         # 查看完整指南
yangzz --doctor                        # 健康检查（排查配置问题）
```

> 💡 **首次运行没有配置？** yangzz 会自动显示配置向导，手把手教你。

---

## ⚙️ 配置

### 中转站（最常见场景）

大多数用户通过中转商使用 API。只需要**中转地址 + Key**：

```toml
provider = "my-relay"
model = "claude-sonnet-4-20250514"

[[providers]]
name = "my-relay"
api_key = "sk-xxxxxx"
base_url = "https://relay.example.com"
default_model = "claude-sonnet-4-20250514"
api_format = "openai"
max_tokens = 16384                     # 单次最大输出（可选）
thinking_budget = 32000                # 思考 token 预算（可选）
context_window = 1000000               # 上下文窗口（可选，主流模型已 1M）
reasoning_effort = "medium"            # 推理强度 low/medium/high（可选）
temperature = 0.7                      # 创造性 0~1（可选）
```

### 多个中转 / 多模型

```toml
provider = "cheap"
model = "deepseek-chat"

# 日常省钱
[[providers]]
name = "cheap"
api_key = "sk-xxx"
base_url = "https://cheap.example.com"
default_model = "deepseek-chat"

# 复杂任务切强模型
[[providers]]
name = "pro"
api_key = "sk-yyy"
base_url = "https://pro.example.com"
default_model = "claude-sonnet-4-20250514"
thinking_budget = 50000
reasoning_effort = "high"

# 本地 Ollama（免费、离线）
[[providers]]
name = "local"
api_key = "ollama"
base_url = "http://localhost:11434"
default_model = "llama3"
```

对话中随时切换：`/model deepseek-chat` → `/model claude-sonnet-4-20250514` → `/model llama3`

### 所有配置项

| 字段 | 说明 | 示例 |
|------|------|------|
| `provider` | 默认 Provider 名 | `"my-relay"` |
| `model` | 默认模型 | `"gpt-4o"` |
| `max_tokens` | 单次最大输出 | `16384` |
| `temperature` | 创造性 0~1 | `0.7` |
| `thinking_budget` | 思考 token 上限 | `32000` |
| `context_window` | 上下文窗口 | `1000000`（主流模型已 1M） |
| `reasoning_effort` | 推理强度 | `"low"` / `"medium"` / `"high"` |

### 配置优先级

```
CLI 参数 > YANGZZ_* 环境变量 > 项目 .yangzz.toml > 全局 config.toml
```

---

## ✨ 功能特性

### 🤖 9 个 Provider

| Provider | API 格式 | 默认模型 |
|----------|---------|---------|
| **OpenAI** | openai | gpt-4o |
| **Anthropic** | anthropic | claude-sonnet-4-20250514 |
| **Google Gemini** | gemini | gemini-2.5-pro |
| **DeepSeek** | openai | deepseek-chat |
| **智谱 GLM** | openai | glm-4-plus |
| **xAI Grok** | openai | grok-3 |
| **Ollama** | openai | llama3 |
| **AWS Bedrock** | bedrock | claude-sonnet-4-20250514 |
| **GCP Vertex** | vertex | gemini-2.5-pro |

> 任何 OpenAI 兼容的中转站都可以直接使用。

### 🛠 17 个内置工具

| 工具 | 功能 |
|------|------|
| `bash` | 执行 shell 命令（沙箱隔离 + 危险命令拦截） |
| `file_read` | 读取文件内容（支持行范围） |
| `file_edit` | 精确替换文件内容 |
| `file_write` | 创建或覆写文件 |
| `file_append` | 追加文件内容 |
| `multi_edit` | 批量多处编辑 |
| `parallel_edit` | 并行编辑多个文件 |
| `grep` | 搜索代码（rg/grep） |
| `glob` | 文件名模式匹配 |
| `list_dir` | 列目录 |
| `tree` | 目录树 |
| `fetch` | HTTP 请求 |
| `notebook` | Jupyter Notebook 操作 |
| `sub_agent` | 子代理（拆分复杂任务） |
| `ask_user` | 向用户提问 |
| `todo` | 任务管理（持久化） |

### 🧠 Hermes 自进化记忆

每次对话结束，Hermes 自动分析你的偏好并写入 `MEMORY.md`：
- 语言偏好（中文/英文）
- 技术栈偏好
- 编码风格（tab vs space、命名规范...）
- 常用框架和工具

下次对话自动加载，**越用越懂你**。

### � 自动记忆捕获

对话中自动识别并记录（零 token 消耗，纯规则匹配）：
- **偏好**：用户说"记住"、"以后"、"不要再"、"我喜欢" → 自动写入
- **教训**：AI 回复含"报错"、"踩坑"、"bug"、"root cause" → 自动记录
- **事实**：对话中出现"项目使用"、"版本"、"端口"、"数据库" → 自动捕获
- **成功**：出现"测试通过"、"部署成功"、"发版成功" → 自动记录

### 🛡 完成度检查

防止 AI 虚报"完成了"：
- 自动检测助手是否声称任务完成
- 校验本轮是否真的执行了文件修改
- 如果没有实际改动却说完成，自动追问

### 🔄 工具失败重试

临时性错误（超时、连接重置等）自动重试一次，无需人工干预。

### 🩺 --doctor 健康检查

```bash
yangzz --doctor
```

自动检查：配置文件、Provider、API Key、工作目录、Rust 工具链等，一目了然排查问题。

### �📊 4 层记忆降级

| 层级 | 上下文使用率 | 策略 |
|------|:----------:|------|
| L0 Full | < 50% | 完整记忆 |
| L1 Summary | 50-80% | 摘要 |
| L2 Keywords | 80-95% | 关键词 |
| L3 None | > 95% | 清空 + Auto Compact |

### 🔒 安全沙箱

- **27 种危险命令拦截**：`rm -rf /`、`DROP TABLE`、`mkfs`、`:(){ :|:& };:` ...
- **符号链接防护**：自动拒绝写入符号链接
- **密钥扫描**：代码中出现 API Key 自动警告
- **沙箱隔离**：配置 `policy.toml` 限制网络、命令、运行时间
- **权限管道**：读操作自动通过，写操作交互确认

### 🎯 智能路由

根据任务复杂度自动选择最佳模型：

```
简单问题 → 便宜模型（DeepSeek）
复杂任务 → 强模型（Claude/GPT）
代码生成 → 推理模型（o3/R1）
```

### 🔍 项目技能检测

首轮对话自动识别项目：
- 语言（Rust/Python/TS/Go...）
- 框架（React/FastAPI/Actix...）
- 包管理器（cargo/npm/pip...）
- 构建工具（webpack/vite/make...）

### 📋 任务队列

```
/task add "修复登录页面 bug"
/task add "重构数据库层"
/task list
/task done 1
```

### 🔄 配置迁移

从其他工具一键迁移：

```
/migrate    ← 自动检测 Claude Code / Codex CLI / Cursor 的配置并导入
```

---

## 📝 REPL 命令

| 命令 | 功能 |
|------|------|
| `/help` | 显示帮助 |
| `/model <name>` | 切换模型 |
| `/model` | 交互式模型选择 |
| `/provider <name>` | 切换 Provider |
| `/undo` | 撤销上次文件编辑（最多 20 次） |
| `/compact` | 压缩对话历史 |
| `/memory [text]` | 查看/添加记忆 |
| `/recall <keyword>` | 搜索过去会话 |
| `/task [cmd]` | 任务队列 |
| `/route <prompt>` | 预览智能路由 |
| `/profile` | 项目技术栈检测 |
| `/policy` | 查看执行策略 |
| `/guide` | 快速上手指南 |
| `/migrate` | 导入其他工具配置 |
| `/status` | Token 用量 + 费用 |
| `/clear` | 清空对话 |
| `/quit` | 退出（自动保存） |

### 快捷键

| 按键 | 功能 |
|------|------|
| `↑/↓` | 翻阅历史输入 |
| `Ctrl+C` | 取消输入 / 中断 AI |
| `Ctrl+D` | 退出 |
| `行尾 \` | 多行输入 |

---

## 🏗 架构

```
yangzz/
├── crates/
│   ├── yangzz-core/           # 核心库
│   │   ├── config/            # 配置加载 + Provider 解析 + 模型适配器
│   │   ├── provider/          # 9 个 Provider 实现 + 智能路由
│   │   ├── tool/              # 17 个内置工具
│   │   ├── query/             # Agentic Loop（记忆降级 + 挫败检测 + JSON修复）
│   │   ├── memory.rs          # 4 层记忆 + Hermes 自进化
│   │   ├── sandbox.rs         # 沙箱 + 执行策略
│   │   ├── skill_detect.rs    # 项目技能检测
│   │   ├── task_queue.rs      # 任务队列
│   │   └── session.rs         # 会话持久化
│   └── yangzz/                # CLI 入口
│       ├── main.rs            # --guide / --setup / REPL / TUI
│       ├── repl.rs            # 交互式对话
│       └── ui/                # 双语 UI + 金色主题
├── npm/                       # npm 发布包
├── sdk/
│   ├── typescript/            # TypeScript SDK
│   └── python/                # Python SDK
```

### 技术栈

- **语言**：Rust（单二进制，启动 <100ms，内存 <20MB）
- **异步**：Tokio
- **HTTP**：reqwest + SSE 流式
- **终端**：rustyline（输入）+ crossterm（TUI）
- **序列化**：serde + toml + serde_json
- **安全**：regex 模式匹配 + 路径遍历防护

---

## 🆚 vs 其他工具

| 特性 | yangzz | Claude Code | Codex CLI |
|------|:------:|:-----------:|:---------:|
| 开源 | ✅ MIT | ❌ 闭源 | ✅ |
| 多 Provider | ✅ 9 个 | ❌ 仅 Claude | ❌ 仅 OpenAI |
| 中转站支持 | ✅ 原生 | ❌ | ❌ |
| 自进化记忆 | ✅ Hermes | ❌ | ❌ |
| 沙箱隔离 | ✅ 内核级 | ✅ | ✅ |
| 任务队列 | ✅ | ❌ | ❌ |
| 智能路由 | ✅ | ❌ | ❌ |
| 配置迁移 | ✅ | — | — |
| 思考深度控制 | ✅ | ❌ | ❌ |
| 中文原生 | ✅ 双语 | ❌ | ❌ |

详细对比见 [COMPARISON.md](../docs/COMPARISON.md)

---

## 📦 SDK

### TypeScript

```typescript
import { Yangzz } from 'yangzz-sdk';

const yz = new Yangzz({ provider: 'my-relay' });
const response = await yz.chat('explain this code');
```

### Python

```python
from yangzz import Yangzz

yz = Yangzz(provider="my-relay")
response = yz.chat("explain this code")
```

---

## 🤝 Contributing

```bash
git clone https://github.com/YangZZtop/yangzz.git
cd yangzz
cargo build
cargo test
cargo run --package yangzz
```

---

## � 致谢

yangzz 的设计和实现参考了以下优秀的开源项目和社区成果：

| 项目 | 启发 |
|------|------|
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code) | REPL 交互范式、工具管道、⎿ 视觉语言 |
| [Codex CLI](https://github.com/openai/codex) (OpenAI) | Agentic Loop 设计、沙箱隔离 |
| [LegnaCode CLI](https://github.com/LegnaOS/LegnaCode-cli) | 多 Provider 抽象、配置迁移思路 |
| [nocode](https://github.com/telagod/nocode) | 终端 AI 助手、Rust 实现参考 |
| [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode) | Hook 系统、Skill 加载机制 |
| [Meta Kim](https://github.com/KimYx0207/Meta_Kim) | 治理守护线、Orchestrator 模式、Meta Agent 架构 |
| [code-yangzz](https://github.com/YangZZtop/code-yangzz) | 前身项目 — 记忆系统、Observatory、编排思路的原型 |
| [Claude Code x OpenClaw Guide](https://github.com/KimYx0207/Claude-Code-x-OpenClaw-Guide-Zh) | 中文社区实践参考 |
| [darwin-skill](https://github.com/alchaincyf/darwin-skill) | Skill 系统设计灵感 |
| [learn-coding-agent](https://github.com/sanbuphy/learn-coding-agent) | Agent Loop 架构学习参考 |
| [huashu-design](https://github.com/alchaincyf/huashu-design) | UI/UX 设计参考 |
| [design.md](https://github.com/google-labs-code/design.md) | 设计规范格式参考 |
| [CCometixLine](https://github.com/Haleclipse/CCometixLine) | Rust 状态栏、Git 集成参考 |

感谢所有参考项目的作者和贡献者。开源社区让我们站在巨人的肩膀上。

---

## �� License

MIT © [yangzz contributors](https://github.com/YangZZtop/yangzz)
