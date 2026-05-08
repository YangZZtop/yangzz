# yangzz

> AI coding assistant — 多模型、多中转、开箱即用

终端 AI 编程助手，支持 9 个 Provider、17 个工具、智能路由、4 层记忆系统。

## 安装

```bash
npm install -g yangzz
```

> `yangzz` 采用 **主包 + 平台子包** 分发。
> 安装主包时，npm 会自动拉取与你当前平台匹配的原生二进制子包，不再依赖 postinstall 去 GitHub 下载。
>
> 如果运行时提示 `Binary not found`，通常是你的包管理器跳过了 `optionalDependencies`：
>
> ```bash
> npm install -g yangzz
> # 仍不行就手动补当前平台包，例如 Apple Silicon:
> npm install -g @yangzz123/yangzz-darwin-arm64
> # 或直接从源码安装:
> cargo install yangzz
> ```

## 配置

创建 `~/.yangzz/config.toml`（Windows 对应 `%USERPROFILE%\.yangzz\config.toml`）：

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
