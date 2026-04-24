use super::presets::{detect_provider_by_key, detect_provider_by_url, find_preset};
use crate::provider::ApiFormat;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application settings — merged from all sources
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub provider: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub api_format: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    /// Thinking/reasoning token budget (for models that support extended thinking)
    pub thinking_budget: Option<u32>,
    /// Override context window size (otherwise auto-detected from model catalog)
    pub context_window: Option<u64>,
    /// Reasoning effort: "low", "medium", "high" (for reasoning models)
    pub reasoning_effort: Option<String>,
    /// Extra providers configured in [[providers]] sections
    #[serde(default)]
    pub providers: Vec<ExtraProvider>,
    /// Multi-agent strategy configuration
    #[serde(default)]
    pub strategy: Option<StrategyConfig>,
}

/// Strategy configuration — user defines which model does what
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StrategyConfig {
    /// "auto" (route by keywords), "manual" (user picks), "team" (parallel multi-agent)
    #[serde(default = "default_strategy_mode")]
    pub mode: String,
    /// Role → provider name mapping (user decides who does what)
    #[serde(default)]
    pub roles: StrategyRoles,
    /// Custom keyword → role mapping (optional override)
    #[serde(default)]
    pub keywords: StrategyKeywords,
}

fn default_strategy_mode() -> String {
    "auto".to_string()
}

/// Maps each role to a provider name from [[providers]]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StrategyRoles {
    /// Architecture planning, system design
    pub planner: Option<String>,
    /// Frontend: React, Vue, CSS, HTML, UI components
    pub frontend: Option<String>,
    /// Backend: API, database, server logic
    pub backend: Option<String>,
    /// Code review, optimization
    pub review: Option<String>,
    /// Testing: unit tests, integration tests
    pub test: Option<String>,
    /// Fallback for unclassified tasks
    pub general: Option<String>,
}

/// Custom keywords per role (user can override defaults)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyKeywords {
    #[serde(default = "default_frontend_keywords")]
    pub frontend: Vec<String>,
    #[serde(default = "default_backend_keywords")]
    pub backend: Vec<String>,
    #[serde(default = "default_review_keywords")]
    pub review: Vec<String>,
    #[serde(default = "default_test_keywords")]
    pub test: Vec<String>,
    #[serde(default = "default_planner_keywords")]
    pub planner: Vec<String>,
}

impl Default for StrategyKeywords {
    fn default() -> Self {
        Self {
            frontend: default_frontend_keywords(),
            backend: default_backend_keywords(),
            review: default_review_keywords(),
            test: default_test_keywords(),
            planner: default_planner_keywords(),
        }
    }
}

fn default_frontend_keywords() -> Vec<String> {
    [
        "react",
        "vue",
        "svelte",
        "css",
        "html",
        "tailwind",
        "component",
        "page",
        "UI",
        "layout",
        "style",
        "组件",
        "页面",
        "前端",
        "样式",
        "布局",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}
fn default_backend_keywords() -> Vec<String> {
    [
        "api",
        "database",
        "server",
        "route",
        "endpoint",
        "migration",
        "sql",
        "redis",
        "queue",
        "接口",
        "后端",
        "数据库",
        "路由",
        "服务",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}
fn default_review_keywords() -> Vec<String> {
    [
        "review", "audit", "check", "optimize", "refine", "improve", "审查", "检查", "优化", "改进",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}
fn default_test_keywords() -> Vec<String> {
    [
        "test",
        "spec",
        "assert",
        "mock",
        "fixture",
        "coverage",
        "测试",
        "断言",
        "覆盖率",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}
fn default_planner_keywords() -> Vec<String> {
    [
        "architecture",
        "design",
        "plan",
        "refactor",
        "migrate",
        "system",
        "架构",
        "设计",
        "规划",
        "重构",
        "迁移",
        "系统",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// An extra provider configured in yangzz config files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtraProvider {
    pub name: String,
    pub api_key: String,
    pub base_url: String,
    pub default_model: Option<String>,
    pub api_format: Option<String>, // "openai" (default) | "anthropic" | "gemini"
    pub thinking_budget: Option<u32>,
    pub context_window: Option<u64>,
    pub reasoning_effort: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

impl Settings {
    /// Load settings with full priority chain:
    /// CLI args > env vars > vendor env vars > config file > presets
    pub fn load(cli_overrides: CliOverrides) -> Self {
        let mut s = Self::default();

        // 1. Try config file (lowest priority of explicit settings)
        if let Some(file_settings) = Self::load_config_file() {
            s.merge(file_settings);
        }

        // 2. Vendor-specific env vars
        s.try_vendor_env_vars();

        // 3. Unified env vars
        s.try_env("YANGZZ_PROVIDER", |s, v| s.provider = Some(v));
        s.try_env("YANGZZ_API_KEY", |s, v| s.api_key = Some(v));
        s.try_env("YANGZZ_MODEL", |s, v| s.model = Some(v));
        s.try_env("YANGZZ_BASE_URL", |s, v| s.base_url = Some(v));
        s.try_env("YANGZZ_API_FORMAT", |s, v| s.api_format = Some(v));

        // 4. CLI args (highest priority)
        if let Some(v) = cli_overrides.provider {
            s.provider = Some(v);
        }
        if let Some(v) = cli_overrides.model {
            s.model = Some(v);
        }
        if let Some(v) = cli_overrides.api_key {
            s.api_key = Some(v);
        }
        if let Some(v) = cli_overrides.base_url {
            s.base_url = Some(v);
        }

        s
    }

    /// Resolve the API format to use — the protocol yangzz speaks to the endpoint.
    ///
    /// `api_format` describes the **wire protocol at the entry**, NOT the model brand:
    ///   - `openai`    → OpenAI-compatible (`/v1/chat/completions`). Covers 99% of relays.
    ///   - `anthropic` → Claude Messages API (`/v1/messages`).
    ///   - `gemini`    → Google Gemini native.
    ///   - `auto`      → same as omitting: infer from URL host, default to OpenAI.
    ///
    /// Key prefixes (like "sk-") are NOT used to guess the protocol — relay keys
    /// happily use "sk-" too. Trust explicit config; fall back to URL host only.
    pub fn resolved_api_format(&self) -> ApiFormat {
        // 1. Explicit format wins. "auto" means "figure it out" → fall through.
        if let Some(ref fmt) = self.api_format {
            match fmt.to_lowercase().as_str() {
                "anthropic" | "claude" => return ApiFormat::Anthropic,
                "gemini" | "google" => return ApiFormat::Gemini,
                "openai" => return ApiFormat::OpenAi,
                "auto" => {}                   // fall through to URL host detection
                _ => return ApiFormat::OpenAi, // unknown value: safest default
            }
        }

        // 2. Provider name matches a built-in preset (e.g. user wrote provider = "anthropic")
        if let Some(ref name) = self.provider {
            if let Some(preset) = find_preset(name) {
                return preset.api_format;
            }
        }

        // 3. URL host matches an **official** vendor domain (api.anthropic.com, etc).
        //    Relays on custom domains fall through → default OpenAI.
        if let Some(ref url) = self.base_url {
            if let Some(preset) = detect_provider_by_url(url) {
                return preset.api_format;
            }
        }

        // 4. Default: OpenAI-compatible. Relays overwhelmingly speak this.
        //    If your relay speaks Anthropic/Gemini, set api_format explicitly.
        ApiFormat::OpenAi
    }

    /// Resolve the base URL
    pub fn resolved_base_url(&self) -> Option<String> {
        if self.base_url.is_some() {
            return self.base_url.clone();
        }

        // From explicit provider
        if let Some(ref name) = self.provider {
            if let Some(preset) = find_preset(name) {
                return Some(preset.base_url.to_string());
            }
        }

        // From key auto-detection
        if let Some(ref key) = self.api_key {
            if let Some(preset) = detect_provider_by_key(key) {
                return Some(preset.base_url.to_string());
            }
        }

        None
    }

    /// Resolve provider display name
    pub fn resolved_provider_name(&self) -> String {
        if let Some(ref name) = self.provider {
            return name.clone();
        }
        if let Some(ref key) = self.api_key {
            if let Some(preset) = detect_provider_by_key(key) {
                return preset.name.to_string();
            }
        }
        "openai".to_string()
    }

    /// Resolve the model to use
    pub fn resolved_model(&self) -> String {
        if let Some(ref m) = self.model {
            return m.clone();
        }
        if let Some(ref name) = self.provider {
            if let Some(preset) = find_preset(name) {
                return preset.default_model.to_string();
            }
        }
        if let Some(ref key) = self.api_key {
            if let Some(preset) = detect_provider_by_key(key) {
                return preset.default_model.to_string();
            }
        }
        "gpt-4o".to_string()
    }

    pub fn resolved_max_tokens(&self) -> u32 {
        self.max_tokens.unwrap_or(16384)
    }

    fn merge(&mut self, other: Self) {
        if self.provider.is_none() {
            self.provider = other.provider;
        }
        if self.api_key.is_none() {
            self.api_key = other.api_key;
        }
        if self.model.is_none() {
            self.model = other.model;
        }
        if self.base_url.is_none() {
            self.base_url = other.base_url;
        }
        if self.api_format.is_none() {
            self.api_format = other.api_format;
        }
        if self.max_tokens.is_none() {
            self.max_tokens = other.max_tokens;
        }
        if self.temperature.is_none() {
            self.temperature = other.temperature;
        }
        if self.thinking_budget.is_none() {
            self.thinking_budget = other.thinking_budget;
        }
        if self.context_window.is_none() {
            self.context_window = other.context_window;
        }
        if self.reasoning_effort.is_none() {
            self.reasoning_effort = other.reasoning_effort;
        }
        if self.providers.is_empty() {
            self.providers = other.providers;
        }
    }

    fn try_env(&mut self, var: &str, apply: fn(&mut Self, String)) {
        if let Ok(val) = std::env::var(var) {
            if !val.is_empty() {
                apply(self, val);
            }
        }
    }

    fn try_vendor_env_vars(&mut self) {
        use super::presets::PRESETS;
        if self.api_key.is_some() {
            return;
        }
        for preset in PRESETS {
            if preset.api_key_env.is_empty() {
                continue;
            }
            if let Ok(key) = std::env::var(preset.api_key_env) {
                if !key.is_empty() {
                    self.api_key = Some(key);
                    if self.provider.is_none() {
                        self.provider = Some(preset.name.to_string());
                    }
                    return;
                }
            }
        }
    }

    fn load_config_file() -> Option<Self> {
        let paths = [
            // Project-local
            PathBuf::from(".yangzz.toml"),
            PathBuf::from(".yangzz/config.toml"),
        ];

        let mut result: Option<Self> = None;

        // Global config first (lowest priority) — unified at ~/.yangzz/config.toml
        let global = crate::paths::config_path();
        if let Ok(content) = std::fs::read_to_string(&global) {
            if let Ok(settings) = toml::from_str::<Self>(&content) {
                result = Some(settings);
            }
        }

        // Project-local overrides global
        for path in &paths {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(local) = toml::from_str::<Self>(&content) {
                    if let Some(ref mut base) = result {
                        // Merge: local providers APPEND to global providers
                        let extra_providers = local.providers.clone();
                        base.merge(local);
                        base.providers.extend(extra_providers);
                    } else {
                        result = Some(local);
                    }
                    break;
                }
            }
        }

        result
    }
}

/// CLI argument overrides
#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}
