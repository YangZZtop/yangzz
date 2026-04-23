use crate::provider::ApiFormat;

/// Built-in provider presets — zero config needed for known providers
pub struct ProviderPreset {
    pub name: &'static str,
    pub api_key_env: &'static str,
    pub key_prefix: &'static str,
    pub base_url: &'static str,
    pub default_model: &'static str,
    pub api_format: ApiFormat,
}

pub const PRESETS: &[ProviderPreset] = &[
    ProviderPreset {
        name: "anthropic",
        api_key_env: "ANTHROPIC_API_KEY",
        key_prefix: "sk-ant-",
        base_url: "https://api.anthropic.com",
        default_model: "claude-sonnet-4-20250514",
        api_format: ApiFormat::Anthropic,
    },
    ProviderPreset {
        name: "openai",
        api_key_env: "OPENAI_API_KEY",
        key_prefix: "sk-",
        base_url: "https://api.openai.com",
        default_model: "gpt-4o",
        api_format: ApiFormat::OpenAi,
    },
    ProviderPreset {
        name: "gemini",
        api_key_env: "GEMINI_API_KEY",
        key_prefix: "AIza",
        base_url: "https://generativelanguage.googleapis.com",
        default_model: "gemini-2.5-pro",
        api_format: ApiFormat::Gemini,
    },
    ProviderPreset {
        name: "deepseek",
        api_key_env: "DEEPSEEK_API_KEY",
        key_prefix: "",
        base_url: "https://api.deepseek.com",
        default_model: "deepseek-chat",
        api_format: ApiFormat::OpenAi,
    },
    ProviderPreset {
        name: "glm",
        api_key_env: "GLM_API_KEY",
        key_prefix: "",
        base_url: "https://open.bigmodel.cn/api/paas/v4",
        default_model: "glm-4-plus",
        api_format: ApiFormat::OpenAi,
    },
    ProviderPreset {
        name: "grok",
        api_key_env: "GROK_API_KEY",
        key_prefix: "xai-",
        base_url: "https://api.x.ai",
        default_model: "grok-3",
        api_format: ApiFormat::OpenAi,
    },
    ProviderPreset {
        name: "ollama",
        api_key_env: "",
        key_prefix: "",
        base_url: "http://localhost:11434",
        default_model: "llama3",
        api_format: ApiFormat::OpenAi,
    },
];

/// Auto-detect provider from API key prefix
pub fn detect_provider_by_key(key: &str) -> Option<&'static ProviderPreset> {
    // Try specific prefixes first (longest match wins)
    let mut candidates: Vec<&ProviderPreset> = PRESETS
        .iter()
        .filter(|p| !p.key_prefix.is_empty() && key.starts_with(p.key_prefix))
        .collect();

    // Sort by prefix length descending — "sk-ant-" should beat "sk-"
    candidates.sort_by(|a, b| b.key_prefix.len().cmp(&a.key_prefix.len()));

    candidates.into_iter().next()
}

/// Find preset by name
pub fn find_preset(name: &str) -> Option<&'static ProviderPreset> {
    PRESETS.iter().find(|p| p.name.eq_ignore_ascii_case(name))
}
