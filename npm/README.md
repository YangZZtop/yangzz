# yangzz

> AI coding assistant — 多模型、多中转、开箱即用

终端 AI 编程助手，支持 9 个 Provider、17 个工具、智能路由、4 层记忆系统。

## 安装

```bash
npm install -g yangzz
```

## 配置

创建 `~/Library/Application Support/yangzz/config.toml`（Mac）或 `~/.config/yangzz/config.toml`（Linux）：

```toml
provider = "my-relay"
model = "claude-sonnet-4-20250514"

[[providers]]
name = "my-relay"
api_key = "sk-你的key"
base_url = "https://你的中转地址"
default_model = "claude-sonnet-4-20250514"
api_format = "openai"
```

## 启动

```bash
yangzz
```

查看完整指南：`yangzz --guide`

## 特性

- **9 个 Provider**：OpenAI / Anthropic / Gemini / DeepSeek / 智谱 / Grok / Ollama / Bedrock / Vertex
- **中转站友好**：填个地址和 key 就能用，不污染环境变量
- **17 个内置工具**：文件读写编辑、bash、grep、子代理、任务管理...
- **4 层记忆系统**：Hermes 自动学习你的偏好
- **智能路由**：按任务复杂度自动选模型
- **安全沙箱**：27 种危险命令拦截 + 内核级隔离
- **中文原生**：双语 UI + Pangu 自动排版

## 也可以用 Cargo 安装

```bash
cargo install yangzz
```
