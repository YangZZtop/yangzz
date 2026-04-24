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

> 😵 **不想看文档？** 直接跳到下面的【🤖 完全不会配？让别的 AI 帮你配】折叠块，把一段 prompt 复制给任何 AI（ChatGPT / Claude / DeepSeek / 豆包都行），它会帮你生成完整配置。跑 `yangzz --setup` 也可以走交互式向导。

### 最小配置示例（中转）

国内用户绝大多数通过中转商使用 API。只需要**中转地址 + key**，再填对 `api_format`。

**`api_format` 怎么选？**（决定 yangzz 用什么协议跟上游说话）

| 你的情况 | `api_format` |
|---|---|
| 中转文档说「**OpenAI 兼容**」/ 用 `/v1/chat/completions` 端点（绝大多数中转） | `openai` |
| 中转文档说「**Anthropic 原生**」/ 用 `/v1/messages` 端点（如 anyrouter、packycode 等） | `anthropic` |
| 官方 Anthropic API 直连（`api.anthropic.com`） | `anthropic` |
| 官方 Gemini 直连（`generativelanguage.googleapis.com`） | `gemini` |
| 拿不准 | `auto`（yangzz 按 URL 自动识别） |

> ⚠️ **不要按模型名猜**。即使调 Claude，只要中转是 OpenAI 兼容的，就填 `openai`。填错了会 404 或 "endpoint not found"，换另一个再试即可。

**配置文件路径**

- macOS: `~/Library/Application Support/yangzz/config.toml`
- Linux: `~/.config/yangzz/config.toml`
- Windows: `%APPDATA%\yangzz\config.toml`

**场景 A：走 GPT 系列（OpenAI 兼容中转）**

```toml
provider = "my-relay"
model = "gpt-4o"

[[providers]]
name = "my-relay"
api_key = "sk-你的key"                          # 中转商给你的 key
base_url = "https://你的中转地址"                # 中转商给你的地址
default_model = "gpt-4o"
api_format = "openai"
```

**场景 B：走 Claude 系列（OpenAI 兼容中转，最常见）**

```toml
provider = "my-relay"
model = "claude-sonnet-4-20250514"

[[providers]]
name = "my-relay"
api_key = "sk-你的key"
base_url = "https://你的中转地址"
default_model = "claude-sonnet-4-20250514"
api_format = "openai"                          # 中转是 OpenAI 兼容协议，不看模型品牌
```

**场景 C：走 Claude（Anthropic 原生中转，如 anyrouter / packycode）**

```toml
provider = "my-relay"
model = "claude-sonnet-4-20250514"

[[providers]]
name = "my-relay"
api_key = "sk-你的key"
base_url = "https://你的中转地址"                # 根域名即可，yangzz 会追加 /v1/messages
default_model = "claude-sonnet-4-20250514"
api_format = "anthropic"                       # Anthropic 原生协议
```

两个中转都想用？写多个 `[[providers]]` 即可，`/model` 随时切换（见下方「多个中转 / 多模型」）。

<details>
<summary><b>📚 常见中转站对照表</b>（点击展开）</summary>

**`base_url` 到底怎么填？**（关键问题，填错最常见）

yangzz 会根据 `api_format` 自动追加端点路径，你**只需要填到根域名 / 路径前缀即可**：

| `api_format` | yangzz 追加的路径 | 你填 `base_url` 填到哪里 |
|---|---|---|
| `openai` | `/v1/chat/completions` | 根域名即可（带不带 `/v1` 都行，代码会归一化） |
| `anthropic` | `/v1/messages` | 只填根域名，**不要带 `/v1`**，否则变成 `/v1/v1/messages` |

❌ 错误示范：`base_url = "https://x.com/v1/chat/completions"`、`base_url = "https://x.com/v1/messages"`
✅ 正确示范：`base_url = "https://x.com"` 或 `base_url = "https://x.com/v1"`（仅 openai 格式下能用）

**常见中转对照**

| 中转站 | 类型 | `api_format` | `base_url` 填法 | 说明 |
|---|---|:---:|---|---|
| **[new-api](https://github.com/Calcium-Ion/new-api)** | 自建 / 托管 | `openai` | `https://你的域名` | 最主流的开源中转，后台可挂多品牌模型 |
| **[one-api](https://github.com/songquanpeng/one-api)** | 自建 / 托管 | `openai` | 同上 | new-api 的前身，配置一致 |
| **sub2api** | 订阅转 API（Claude Pro/Max） | `openai` 或 `anthropic` | 根域名即可（两种端点都在同一域名下） | 把 Claude 订阅转成 API，两种协议都支持 |
| **cc-switch** | 多账号 key 切换工具 | — | — | 不是中转，是本地切换工具，跟 yangzz 的 `/provider` 定位重叠 |
| **anyrouter / packycode** | 商业 Anthropic 中转 | `anthropic` | 商家给的根域名（不要带 `/v1`） | 原生 Anthropic 协议，工具调用 / thinking 最接近官方 |
| **通用 OpenAI 中转**（oaipro、chatanywhere、deepbricks 等） | 商业 | `openai` | 商家给的根域名 | 绝大多数走这条 |
| **Ollama 本地** | 本地 | `openai` | `http://localhost:11434` | 本地模型，不用 key |

**new-api 典型配置**

```toml
provider = "newapi"
model = "claude-sonnet-4-20250514"

[[providers]]
name = "newapi"
api_key = "sk-xxx"                                # new-api 后台给你生成的 key
base_url = "https://newapi.你的域名.com"           # 根域名，yangzz 会自动追加 /v1/chat/completions
default_model = "claude-sonnet-4-20250514"        # 填 new-api 后台里那个模型名
api_format = "openai"
```

**sub2api 典型配置（两种端点二选一）**

```toml
# —— 方式 1：走 OpenAI 端点 ——
provider = "sub2api"
model = "claude-sonnet-4-20250514"

[[providers]]
name = "sub2api"
api_key = "sk-你的sub2api-key"
base_url = "https://你的sub2api"                  # 根域名即可，yangzz 会追加 /v1/chat/completions
default_model = "claude-sonnet-4-20250514"
api_format = "openai"

# —— 方式 2：走 Anthropic 原生端点（推荐，工具调用更稳） ——
# 把 api_format 改成 "anthropic"，base_url 保持根域名不变
# yangzz 会自动追加 /v1/messages
```

**anyrouter / packycode 典型配置**

```toml
provider = "anyrouter"
model = "claude-sonnet-4-20250514"

[[providers]]
name = "anyrouter"
api_key = "sk-xxx"
base_url = "https://你拿到的anyrouter地址"          # 根域名，不要带 /v1
default_model = "claude-sonnet-4-20250514"
api_format = "anthropic"                          # 原生 Anthropic 协议
```

> 💡 **排错口诀**：
> - **404 / endpoint not found** → 十有八九是 `api_format` 填反了，或 `base_url` 里带了 `/v1/chat/completions` 这种终点路径
> - **401 / unauthorized** → key 错或 key 没权限
> - **连接超时** → `base_url` 域名不通，先用 `curl $base_url` 测

</details>

### 安装

```bash
# npm（推荐）
npm install -g yangzz

# 或 Cargo
cargo install --path crates/yangzz
```

### 一键配置

**把下面脚本中的 3 个值改成你自己的，复制粘贴到终端执行即可：**

> 💡 脚本默认用 `api_format = "openai"`，覆盖绝大多数中转。如果你的中转是 **Anthropic 原生**（如 anyrouter、packycode，URL 带 `/v1/messages`），跑完脚本后手动把配置文件里那一行改成 `api_format = "anthropic"`。详见 [api_format 决策表](#最小配置示例中转)。

<details>
<summary><b>🍎 macOS / 🐧 Linux</b>（点击展开）</summary>

```bash
# ⚠️ 只需要改这 3 行 ⚠️
MY_KEY="sk-你的key"                        # 中转商给你的 key
MY_URL="https://你的中转地址"               # 中转商给你的地址
MY_MODEL="claude-sonnet-4-20250514"        # 默认模型（改成你想用的，如 gpt-4o、deepseek-chat 等）

# —— 以下不用动 ——
if [ "$(uname)" = "Darwin" ]; then
  DIR="$HOME/Library/Application Support/yangzz"
else
  DIR="$HOME/.config/yangzz"
fi
mkdir -p "$DIR"
cat > "$DIR/config.toml" << EOF
provider = "my-relay"
model = "$MY_MODEL"

[[providers]]
name = "my-relay"
api_key = "$MY_KEY"
base_url = "$MY_URL"
default_model = "$MY_MODEL"
# api_format 指的是中转商的接口协议，不是模型品牌
# 国内中转站几乎都用 OpenAI 兼容协议，即使调 Claude/DeepSeek 也填 openai
api_format = "openai"
EOF
echo "✅ 配置已写入: $DIR/config.toml"
echo "🚀 现在运行 yangzz 即可开始！"
```

</details>

<details>
<summary><b>🪟 Windows PowerShell</b>（点击展开）</summary>

```powershell
# ⚠️ 只需要改这 3 行 ⚠️
$MY_KEY = "sk-你的key"                       # 中转商给你的 key
$MY_URL = "https://你的中转地址"              # 中转商给你的地址
$MY_MODEL = "claude-sonnet-4-20250514"       # 默认模型（改成你想用的，如 gpt-4o、deepseek-chat 等）

# —— 以下不用动 ——
$dir = "$env:APPDATA\yangzz"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
@"
provider = "my-relay"
model = "$MY_MODEL"

[[providers]]
name = "my-relay"
api_key = "$MY_KEY"
base_url = "$MY_URL"
default_model = "$MY_MODEL"
# api_format 指的是中转商的接口协议，不是模型品牌
# 国内中转站几乎都用 OpenAI 兼容协议，即使调 Claude/DeepSeek 也填 openai
api_format = "openai"
"@ | Out-File -Encoding utf8 "$dir\config.toml"
Write-Host "✅ 配置已写入: $dir\config.toml"
Write-Host "🚀 现在运行 yangzz 即可开始！"
```

</details>

<details>
<summary><b>🔀 多个中转？（如 A 中转用 GPT，B 中转用 Claude）</b>（点击展开）</summary>

写多个 `[[providers]]` 即可，对话中 `/model` 随时切换：

```bash
# ⚠️ 改成你自己的 ⚠️
A_KEY="sk-aaa"                             # A 中转的 key
A_URL="https://a-relay.example.com"        # A 中转的地址
B_KEY="sk-bbb"                             # B 中转的 key
B_URL="https://b-relay.example.com"        # B 中转的地址

# —— 以下不用动 ——
if [ "$(uname)" = "Darwin" ]; then
  DIR="$HOME/Library/Application Support/yangzz"
else
  DIR="$HOME/.config/yangzz"
fi
mkdir -p "$DIR"
cat > "$DIR/config.toml" << EOF
provider = "a-relay"
model = "gpt-4o"

# A 中转（GPT 系列）
[[providers]]
name = "a-relay"
api_key = "$A_KEY"
base_url = "$A_URL"
default_model = "gpt-4o"
api_format = "openai"

# B 中转（Claude 系列）
[[providers]]
name = "b-relay"
api_key = "$B_KEY"
base_url = "$B_URL"
default_model = "claude-sonnet-4-20250514"
api_format = "openai"
EOF
echo "✅ 配置已写入: $DIR/config.toml"
echo "🚀 对话中用 /model 切换模型，yangzz 会自动匹配对应的中转"
```

切换方式：
```
/model gpt-4o                    ← 自动走 A 中转
/model claude-sonnet-4-20250514  ← 自动走 B 中转
/model deepseek-chat             ← 如果还配了 C 中转，自动走 C
```

</details>

> 💡 配完后可以运行 `yangzz --doctor` 检查配置是否正确。

<details>
<summary><b>🤖 完全不会配？让别的 AI 帮你配（小白专用）</b></summary>

**你只需要做一件事**：把下面这段文字 + 你自己的信息，整段复制到任何一个 AI 对话框里（ChatGPT、Claude、DeepSeek、豆包、Kimi、通义、文心一言都行），它会输出完整配置和写入命令，照做即可。

```text
我要用 yangzz（Rust 写的终端 AI 编程助手）连接我的中转站。请帮我生成配置文件 config.toml 的内容，并给出写入文件的完整命令。

yangzz 的配置规则（你必须遵守）：

1. 配置文件路径：
   - macOS: ~/Library/Application Support/yangzz/config.toml
   - Linux: ~/.config/yangzz/config.toml
   - Windows: %APPDATA%\yangzz\config.toml

2. 配置文件格式是 TOML，必须包含：
   - 顶层 provider = "随便起的名字"
   - 顶层 model = "默认模型名"
   - 一个或多个 [[providers]] 数组项，每项有 name、api_key、base_url、default_model、api_format 五个字段

3. api_format 取值规则（只能四选一）：
   - "openai"：中转走 /v1/chat/completions 端点（国内绝大多数中转、new-api、one-api、sub2api 的 OpenAI 模式）
   - "anthropic"：中转走 /v1/messages 端点（anyrouter、packycode、sub2api 的 Anthropic 模式、官方 api.anthropic.com）
   - "gemini"：官方 generativelanguage.googleapis.com
   - "auto"：不确定就填这个，yangzz 按 URL 自动识别

4. base_url 规则（非常重要，填错会 404）：
   - 只填中转的根域名，例如 "https://myrelay.com"
   - 绝对不要带 /v1/chat/completions、/v1/messages、/chat/completions 这种终点路径
   - openai 格式下 base_url 带不带 /v1 都行，yangzz 会自动归一化
   - anthropic 格式下 base_url 不要带 /v1，否则会变成 /v1/v1/messages

5. 重要心智：api_format 看中转协议，不看模型品牌。即使用 Claude，只要中转是 OpenAI 兼容协议，就填 "openai"。

我的信息：
- 操作系统：<填 macOS / Linux / Windows>
- 中转地址（base_url）：<填你中转商给你的地址>
- 中转类型：<如果你知道的话，比如 "new-api 自建"、"anyrouter 商业 Anthropic 原生中转"；不知道就写"不确定">
- API Key：<用 SK_PLACEHOLDER 代替，不要给 AI 看真 key>
- 想用的默认模型：<填，比如 claude-sonnet-4-20250514、gpt-4o、deepseek-chat 等>

请输出两样东西：
① 完整的 config.toml 内容（代码块）
② 针对我操作系统的一行命令，用来把配置写入正确位置（代码块）
```

**使用步骤**：

1. 复制上面整段文字到 AI 对话框（比如把这段发给 ChatGPT 或 Claude）
2. 把文字里 `<填...>` 那些占位符换成你的真实信息
3. AI 会输出 `config.toml` 内容和一行写入命令
4. **真实 API Key 不要给 AI 看**。AI 输出的命令里如果出现 `SK_PLACEHOLDER`，你自己手动改成真 key，再粘贴到终端执行
5. 跑 `yangzz --doctor` 验证，再跑 `yangzz` 开聊

> ⚠️ 永远不要把真实 API Key 发给第三方 AI。用占位符 + 本地手动替换，比什么都安全。

</details>

<details>
<summary>手动配置（不用脚本）</summary>

配置文件位置：
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

</details>

### 启动

```bash
yangzz                                 # 默认 REPL（推荐）
yangzz "fix the bug in src/main.rs"    # 单次执行
yangzz --tui                           # 实验性全屏 TUI
yangzz --guide                         # 查看完整指南
yangzz --setup                         # 重新运行配置向导
yangzz --doctor                        # 健康检查（排查配置问题）
yangzz --where                         # 打印所有数据路径（config / session / memory）
yangzz --uninstall                     # 交互式卸载（清理配置、会话、记忆）
```

> 💡 **默认是经典 REPL。** 保留原生终端滚动、复制、选择；`--tui` 仍可用，但当前是实验态。
>
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

| Provider | API 格式 | 默认模型 | 备注 |
|----------|---------|---------|------|
| **OpenAI** | openai | gpt-4o | 官方 |
| **Anthropic** | anthropic | claude-sonnet-4-20250514 | 官方 |
| **Google Gemini** | gemini | gemini-2.5-pro | 官方 |
| **DeepSeek** | openai | deepseek-chat | 官方 |
| **智谱 GLM** | openai | glm-4-plus | 官方 |
| **xAI Grok** | openai | grok-3 | 官方 |
| **Ollama** | openai | llama3 | 本地离线 |
| **AWS Bedrock** | bedrock | claude-sonnet-4-20250514 | 需显式配置 |
| **GCP Vertex** | vertex | gemini-2.5-pro | 需显式配置 |

> 任何 OpenAI 兼容的中转站都可以直接使用。`/model` 只会列出你显式配置过的 provider，不会把未配置的 preset 混进来。

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

### 📝 自动记忆捕获

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

### 📊 4 层记忆降级

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

v0.3.1 起斜杠命令采用**名词优先 + 子命令**的统一语法。`/help` 按类别分组展示，`/help <名字>` 查看单个命令的详细用法。

### 配置类

| 命令 | 说明 |
|------|------|
| `/model` | 交互式选择模型（只列出已配置的 provider） |
| `/model <name>` | 直接切换到指定模型 |
| `/provider` | 列出已配置的 provider |
| `/provider add` | 交互式添加新 provider |
| `/provider edit <name>` | 修改某个 provider 的字段 |
| `/provider rename <old> <new>` | 重命名 provider |
| `/provider remove <name>` | 删除 provider |
| `/provider <name>` | 切换到指定 provider |
| `/key list` | 列出所有 provider 的 API key 状态（不显示明文） |
| `/key set <provider>` | 更新某 provider 的 API key |
| `/config` | 查看当前生效的配置 |
| `/config path` | 打印配置文件路径 |

### 对话类

| 命令 | 说明 |
|------|------|
| `/clear` | 清空对话历史 |
| `/compact` | 压缩历史（保留要点） |
| `/undo` | 撤销上次文件编辑（最多 20 次） |
| `/memory [text]` | 查看/追加 `MEMORY.md` |
| `/recall <keyword>` | 搜索历史会话 |
| `/status` | 本轮 token 用量 + 费用 |

### 扩展类

| 命令 | 说明 |
|------|------|
| `/mcp` | 列出已接入的 MCP server |
| `/mcp add` | 交互式接入 MCP server |
| `/mcp remove <name>` | 移除 MCP server |
| `/tool` | 列出所有可用工具（含 MCP 外部工具） |
| `/skill` | 查看/管理 skill（单项） |
| `/skills` | 列出全部 skill |

### 任务类

| 命令 | 说明 |
|------|------|
| `/task` | 任务队列操作（`add` / `list` / `done` / `cancel`） |
| `/route <prompt>` | 预览智能路由会选哪个模型 |
| `/strategy` | 多模型协作策略配置 |
| `/profile` | 自动识别的项目技术栈 |
| `/policy` | 当前执行策略（沙箱 / 网络 / 超时） |

### 其他

| 命令 | 说明 |
|------|------|
| `/help` / `/help <name>` | 显示帮助 / 查看单条命令详情 |
| `/guide` | 快速上手指南 |
| `/migrate` | 从 Claude Code / Codex / Cursor 导入配置 |
| `/quit` | 退出（自动保存会话） |

> 💡 **REPL 和 CLI 共用同一个命令处理器**：`yangzz provider add` 和 `/provider add` 走同一份代码。

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
│   │   ├── query/             # Agentic Loop（记忆降级 + 挫败检测 + JSON 修复）
│   │   ├── memory.rs          # 4 层记忆 + Hermes 自进化
│   │   ├── sandbox.rs         # 沙箱 + 执行策略
│   │   ├── skill_detect.rs    # 项目技能检测
│   │   ├── task_queue.rs      # 任务队列
│   │   ├── attach.rs          # @path 附件解析（图片 / 文件）
│   │   └── session.rs         # 会话持久化
│   └── yangzz/                # CLI 入口
│       ├── main.rs            # --guide / --setup / --doctor / --where / --uninstall / 默认 REPL
│       ├── repl.rs            # 主产品交互路径
│       ├── repl_render.rs     # 流式渲染 + Markdown + 霓虹色板
│       ├── repl_help.rs       # /help 分类展示
│       ├── slash/             # 斜杠命令系统（名词优先 + 子命令）
│       │   ├── mod.rs         # SlashCommand trait + Registry
│       │   ├── wizard.rs      # 首次运行向导
│       │   └── commands/      # provider / key / config / mcp / task / skill / ...
│       ├── tui/               # 实验性全屏 TUI
│       └── ui/                # 色板 / banner / 状态栏 / i18n
├── npm/                       # npm 发布包
├── sdk/
│   ├── typescript/            # TypeScript SDK
│   └── python/                # Python SDK
```

### 技术栈

- **语言**：Rust（单二进制，启动 <100ms，内存 <20MB）
- **异步**：Tokio
- **HTTP**：reqwest + SSE 流式
- **终端**：rustyline（默认 REPL）+ crossterm / ratatui（实验 TUI）
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

## 🙏 致谢

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

## 📜 License

MIT © [yangzz contributors](https://github.com/YangZZtop/yangzz)
