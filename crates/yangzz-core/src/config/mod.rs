pub mod model_meta;
mod presets;
pub mod settings;
pub mod writer;

pub use presets::{PRESETS, ProviderPreset};
pub use settings::Settings;

use crate::provider::{
    AnthropicProvider, ApiFormat, OpenAiCompatProvider, Provider, ProviderError,
};
use std::sync::Arc;

/// A provider with its name and API key, ready to be queried for models
pub struct AvailableProvider {
    pub name: String,
    pub provider: Arc<dyn Provider>,
}

/// Infer the provider family from a model id or provider-like alias.
pub fn detect_provider_family(value: &str) -> Option<&'static str> {
    let lower = value.to_lowercase();
    if lower.starts_with("claude") || lower == "anthropic" {
        Some("anthropic")
    } else if lower.starts_with("mimo") || lower == "xiaomi" || value == "小米" {
        Some("xiaomi")
    } else if lower.starts_with("gpt")
        || lower.starts_with("o3")
        || lower.starts_with("o4")
        || lower.starts_with("o1")
        || lower == "openai"
    {
        Some("openai")
    } else if lower.starts_with("gemini") || lower == "gemini" {
        Some("gemini")
    } else if lower.starts_with("deepseek") || lower == "deepseek" {
        Some("deepseek")
    } else if lower.starts_with("glm") || lower == "glm" {
        Some("glm")
    } else if lower.starts_with("grok") || lower == "grok" {
        Some("grok")
    } else if lower.starts_with("llama")
        || lower.starts_with("qwen")
        || lower.starts_with("mistral")
        || lower.starts_with("phi")
        || lower == "ollama"
    {
        Some("ollama")
    } else {
        None
    }
}

/// Pick the best configured provider name for a target model.
///
/// Preference order:
/// 1. exact default_model match
/// 2. same family as target model
/// 3. prefer current provider when it already matches the same family
pub fn select_provider_name_for_model(
    settings: &Settings,
    model: &str,
    current_provider: Option<&str>,
) -> Option<String> {
    let family = detect_provider_family(model)?;
    select_provider_name_for_family(settings, family, Some(model), current_provider)
}

fn select_provider_name_for_family(
    settings: &Settings,
    family: &str,
    requested_model: Option<&str>,
    current_provider: Option<&str>,
) -> Option<String> {
    settings
        .providers
        .iter()
        .filter_map(|extra| {
            let mut score = 0usize;
            let default_model = extra.default_model.as_deref();

            if let (Some(default_model), Some(requested_model)) = (default_model, requested_model) {
                if default_model.eq_ignore_ascii_case(requested_model) {
                    score = score.max(100);
                }
            }

            if default_model
                .and_then(detect_provider_family)
                .is_some_and(|candidate| candidate == family)
            {
                score = score.max(60);
            }

            if detect_provider_family(&extra.name).is_some_and(|candidate| candidate == family) {
                score = score.max(30);
            }

            if matches!(extra.api_format.as_deref(), Some("anthropic")) && family == "anthropic" {
                score = score.max(20);
            }

            if matches!(extra.api_format.as_deref(), Some("gemini")) && family == "gemini" {
                score = score.max(20);
            }

            if score > 0
                && current_provider.is_some_and(|current| extra.name.eq_ignore_ascii_case(current))
            {
                score += 10;
            }

            (score > 0).then(|| (score, extra.name.clone()))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, name)| name)
}

/// Detect all providers that have API keys configured.
/// Returns a list of (name, provider) that can call list_models().
/// Also includes the current provider if it uses a custom base_url (YANGZZ_*).
pub fn list_available_providers(
    current_provider: Option<&Arc<dyn Provider>>,
    settings: &Settings,
) -> Vec<AvailableProvider> {
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Current provider ALWAYS comes first (it's what the user is actively using)
    if let Some(cur) = current_provider {
        let name = cur.name().to_string();
        seen.insert(name.clone());
        result.push(AvailableProvider {
            name,
            provider: Arc::clone(cur),
        });
    }

    // Extra providers from config files ([[providers]] sections)
    for extra in &settings.providers {
        if seen.contains(&extra.name) {
            continue;
        }
        let fmt = extra.api_format.as_deref().unwrap_or("openai");
        let provider: Option<Arc<dyn Provider>> = match fmt {
            "anthropic" => AnthropicProvider::with_base_url(
                &extra.api_key,
                &extra.base_url,
                extra.default_model.clone(),
            )
            .ok()
            .map(|p| Arc::new(p) as Arc<dyn Provider>),
            _ => OpenAiCompatProvider::new(
                &extra.name,
                &extra.api_key,
                &extra.base_url,
                extra.default_model.clone(),
            )
            .ok()
            .map(|p| Arc::new(p) as Arc<dyn Provider>),
        };
        if let Some(p) = provider {
            seen.insert(extra.name.clone());
            result.push(AvailableProvider {
                name: extra.name.clone(),
                provider: p,
            });
        }
    }

    // Built-in presets (including keyless ones like xiaomi / ollama)
    for preset in PRESETS {
        if seen.contains(preset.name) {
            continue;
        }
        let key = if preset.api_key_env.is_empty() {
            String::new()
        } else {
            let key = std::env::var(preset.api_key_env).unwrap_or_default();
            if key.is_empty() {
                continue;
            }
            key
        };
        let provider: Option<Arc<dyn Provider>> = match preset.api_format {
            ApiFormat::Anthropic => AnthropicProvider::new(&key, None)
                .ok()
                .map(|p| Arc::new(p) as Arc<dyn Provider>),
            ApiFormat::OpenAi | ApiFormat::Gemini => OpenAiCompatProvider::new(
                preset.name,
                &key,
                preset.base_url,
                Some(preset.default_model.to_string()),
            )
            .ok()
            .map(|p| Arc::new(p) as Arc<dyn Provider>),
        };
        if let Some(p) = provider {
            seen.insert(preset.name.to_string());
            result.push(AvailableProvider {
                name: preset.name.to_string(),
                provider: p,
            });
        }
    }

    result
}

/// Keep only providers explicitly declared in `[[providers]]`.
///
/// If the user has no explicit providers configured, preserve the incoming
/// list so `/model` still works for env-only / preset-only setups.
pub fn retain_configured_providers(
    providers: Vec<AvailableProvider>,
    settings: &Settings,
) -> Vec<AvailableProvider> {
    let configured_names: std::collections::HashSet<String> = settings
        .providers
        .iter()
        .map(|provider| provider.name.to_lowercase())
        .collect();

    if configured_names.is_empty() {
        return providers;
    }

    providers
        .into_iter()
        .filter(|provider| configured_names.contains(&provider.name.to_lowercase()))
        .collect()
}

/// Fallback model list used when a provider cannot return `/v1/models`.
///
/// Preference order:
/// 1. known provider-specific static catalog (currently xiaomi)
/// 2. configured default_model for the named provider
/// 3. provider.default_model()
pub fn fallback_models_for_provider(
    settings: &Settings,
    provider_name: &str,
    provider_default_model: &str,
) -> Vec<String> {
    let mut models = Vec::new();

    let configured_default = settings
        .providers
        .iter()
        .find(|provider| provider.name.eq_ignore_ascii_case(provider_name))
        .and_then(|provider| provider.default_model.clone())
        .or_else(|| {
            PRESETS
                .iter()
                .find(|preset| preset.name.eq_ignore_ascii_case(provider_name))
                .map(|preset| preset.default_model.to_string())
        });

    if detect_provider_family(provider_name) == Some("xiaomi")
        || configured_default
            .as_deref()
            .and_then(detect_provider_family)
            .is_some_and(|family| family == "xiaomi")
        || detect_provider_family(provider_default_model) == Some("xiaomi")
    {
        for model in [
            "mimo-v2-flash",
            "mimo-v2-omni",
            "mimo-v2-pro",
            "mimo-v2-tts",
            "mimo-v2.5",
            "mimo-v2.5-pro",
            "mimo-v2.5-tts",
            "mimo-v2.5-tts-voiceclone",
            "mimo-v2.5-tts-voicedesign",
        ] {
            push_unique_model(&mut models, model.to_string());
        }
    }

    if let Some(model) = configured_default {
        push_unique_model(&mut models, model);
    }

    if !provider_default_model.is_empty() {
        push_unique_model(&mut models, provider_default_model.to_string());
    }

    models.sort();
    models
}

fn push_unique_model(models: &mut Vec<String>, model: String) {
    if !model.is_empty() && !models.iter().any(|existing| existing == &model) {
        models.push(model);
    }
}

/// Resolve a Provider from settings — the "轻配置" magic
///
/// Priority:
///   1. CLI args (provider, model, base_url)
///   2. Environment variables (YANGZZ_API_KEY, YANGZZ_MODEL, etc.)
///   3. Vendor-specific env vars (ANTHROPIC_API_KEY, OPENAI_API_KEY, etc.)
///   4. Config file settings
///   5. Built-in presets
pub fn resolve_provider(settings: &Settings) -> Result<Arc<dyn Provider>, ProviderError> {
    let build_provider = |provider_name: &str,
                          api_key: &str,
                          base_url: &str,
                          model: Option<String>,
                          format: ApiFormat|
     -> Result<Arc<dyn Provider>, ProviderError> {
        match format {
            ApiFormat::Anthropic => {
                let provider = AnthropicProvider::with_base_url(api_key, base_url, model)?;
                Ok(Arc::new(provider))
            }
            ApiFormat::OpenAi | ApiFormat::Gemini => {
                let provider = OpenAiCompatProvider::new(provider_name, api_key, base_url, model)?;
                Ok(Arc::new(provider))
            }
        }
    };

    // First: try to match provider name against [[providers]] in config
    if let Some(ref provider_name) = settings.provider {
        for extra in &settings.providers {
            if extra.name.eq_ignore_ascii_case(provider_name) {
                let fmt = extra.api_format.as_deref().unwrap_or("openai");
                let model = settings
                    .model
                    .clone()
                    .or_else(|| extra.default_model.clone());
                let format = if fmt.eq_ignore_ascii_case("anthropic") {
                    ApiFormat::Anthropic
                } else {
                    ApiFormat::OpenAi
                };
                return build_provider(&extra.name, &extra.api_key, &extra.base_url, model, format);
            }
        }

        if let Some(family) = detect_provider_family(provider_name) {
            if let Some(matched_name) =
                select_provider_name_for_family(settings, family, settings.model.as_deref(), None)
            {
                if let Some(extra) = settings
                    .providers
                    .iter()
                    .find(|extra| extra.name.eq_ignore_ascii_case(&matched_name))
                {
                    let fmt = extra.api_format.as_deref().unwrap_or("openai");
                    let model = settings
                        .model
                        .clone()
                        .or_else(|| extra.default_model.clone());
                    let format = if fmt.eq_ignore_ascii_case("anthropic") {
                        ApiFormat::Anthropic
                    } else {
                        ApiFormat::OpenAi
                    };
                    return build_provider(
                        &extra.name,
                        &extra.api_key,
                        &extra.base_url,
                        model,
                        format,
                    );
                }
            }
        }
    }

    // Fallback: env vars / presets (original logic)
    let api_key = settings.api_key.as_deref().unwrap_or("");

    // Keyless built-in preset (e.g. xiaomi / ollama)
    if api_key.is_empty() {
        if let Some(provider_name) = settings.provider.as_deref() {
            if let Some(preset) = PRESETS.iter().find(|preset| {
                preset.name.eq_ignore_ascii_case(provider_name) && preset.api_key_env.is_empty()
            }) {
                let base_url = settings.base_url.as_deref().unwrap_or(preset.base_url);
                let model = settings
                    .model
                    .clone()
                    .or_else(|| Some(preset.default_model.to_string()));
                return build_provider(preset.name, "", base_url, model, preset.api_format);
            }
        }

        // Public / relay endpoint explicitly configured at the top level.
        if let Some(url) = settings.base_url.as_deref() {
            let provider_name = settings.resolved_provider_name();
            let model = settings.model.clone();
            return build_provider(
                &provider_name,
                "",
                url,
                model,
                settings.resolved_api_format(),
            );
        }

        // If no explicit key but we have providers in config, use the first one
        if let Some(first) = settings.providers.first() {
            let fmt = first.api_format.as_deref().unwrap_or("openai");
            let model = settings
                .model
                .clone()
                .or_else(|| first.default_model.clone());
            let format = if fmt.eq_ignore_ascii_case("anthropic") {
                ApiFormat::Anthropic
            } else {
                ApiFormat::OpenAi
            };
            return build_provider(&first.name, &first.api_key, &first.base_url, model, format);
        }
        return Err(ProviderError::Auth(
            "No API key found. Set YANGZZ_API_KEY or configure [[providers]] in config.toml."
                .into(),
        ));
    }

    let format = settings.resolved_api_format();
    let base_url = settings.resolved_base_url();
    let model = settings.model.clone();

    match format {
        ApiFormat::Anthropic => {
            let provider = if let Some(ref url) = base_url {
                AnthropicProvider::with_base_url(api_key, url, model)?
            } else {
                AnthropicProvider::new(api_key, model)?
            };
            Ok(Arc::new(provider))
        }
        ApiFormat::OpenAi | ApiFormat::Gemini => {
            let provider_name = settings.resolved_provider_name();
            let url = base_url.unwrap_or_else(|| "https://api.openai.com".to_string());
            let provider = OpenAiCompatProvider::new(&provider_name, api_key, &url, model)?;
            Ok(Arc::new(provider))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Settings, detect_provider_family, fallback_models_for_provider,
        list_available_providers, resolve_provider, retain_configured_providers,
        select_provider_name_for_model,
    };
    use crate::config::settings::ExtraProvider;

    #[test]
    fn detect_provider_family_from_common_model_names() {
        assert_eq!(detect_provider_family("claude-sonnet-4"), Some("anthropic"));
        assert_eq!(detect_provider_family("mimo-v2.5-pro"), Some("xiaomi"));
        assert_eq!(detect_provider_family("gpt-4o"), Some("openai"));
        assert_eq!(detect_provider_family("gemini-2.5-pro"), Some("gemini"));
        assert_eq!(detect_provider_family("deepseek-chat"), Some("deepseek"));
        assert_eq!(detect_provider_family("qwen2.5"), Some("ollama"));
        assert_eq!(detect_provider_family("mystery-model"), None);
    }

    #[test]
    fn select_provider_name_for_model_prefers_matching_family_and_current_provider() {
        let settings = Settings {
            providers: vec![
                extra_provider("cheap-openai", "gpt-4o-mini", "openai"),
                extra_provider("pro-openai", "gpt-5.4", "openai"),
                extra_provider("claude-relay", "claude-sonnet-4-20250514", "openai"),
            ],
            ..Settings::default()
        };

        let selected =
            select_provider_name_for_model(&settings, "gpt-4.1", Some("pro-openai")).unwrap();

        assert_eq!(selected, "pro-openai");
    }

    #[test]
    fn resolve_provider_maps_family_alias_to_named_custom_provider() {
        let settings = Settings {
            provider: Some("openai".into()),
            model: Some("gpt-4o".into()),
            providers: vec![
                extra_provider("claude-relay", "claude-sonnet-4-20250514", "openai"),
                extra_provider("gpt-relay", "gpt-4o-mini", "openai"),
            ],
            ..Settings::default()
        };

        let provider = resolve_provider(&settings).unwrap();

        assert_eq!(provider.name(), "gpt-relay");
        assert_eq!(provider.default_model(), "gpt-4o");
    }

    #[test]
    fn resolve_provider_supports_keyless_xiaomi_preset() {
        let settings = Settings {
            provider: Some("xiaomi".into()),
            model: Some("mimo-v2.5-pro".into()),
            ..Settings::default()
        };

        let provider = resolve_provider(&settings).unwrap();

        assert_eq!(provider.name(), "xiaomi");
        assert_eq!(provider.default_model(), "mimo-v2.5-pro");
    }

    #[test]
    fn list_available_providers_includes_keyless_xiaomi_preset() {
        let settings = Settings::default();

        let providers = list_available_providers(None, &settings);

        assert!(providers.iter().any(|provider| provider.name == "xiaomi"));
    }

    #[test]
    fn fallback_models_for_provider_returns_full_xiaomi_catalog() {
        let settings = Settings::default();

        let models = fallback_models_for_provider(&settings, "xiaomi", "mimo-v2.5-pro");

        assert!(models.contains(&"mimo-v2-flash".to_string()));
        assert!(models.contains(&"mimo-v2.5-pro".to_string()));
        assert!(models.contains(&"mimo-v2.5-tts-voicedesign".to_string()));
        assert_eq!(models.len(), 9);
    }

    #[test]
    fn retain_configured_providers_hides_builtin_presets_when_custom_config_exists() {
        let settings = Settings {
            providers: vec![
                extra_provider("薯条GPT-pro", "gpt-5.4", "openai"),
                extra_provider("xiaomi", "mimo-v2.5-pro", "openai"),
            ],
            ..Settings::default()
        };

        let providers = list_available_providers(None, &settings);
        let filtered = retain_configured_providers(providers, &settings);
        let names: Vec<String> = filtered.into_iter().map(|provider| provider.name).collect();

        assert!(names.iter().any(|name| name == "薯条GPT-pro"));
        assert!(names.iter().any(|name| name == "xiaomi"));
        assert!(!names.iter().any(|name| name == "ollama"));
    }

    fn extra_provider(name: &str, default_model: &str, api_format: &str) -> ExtraProvider {
        ExtraProvider {
            name: name.into(),
            api_key: "sk-test".into(),
            base_url: "https://example.com/v1".into(),
            default_model: Some(default_model.into()),
            api_format: Some(api_format.into()),
            thinking_budget: None,
            context_window: None,
            reasoning_effort: None,
            max_tokens: None,
            temperature: None,
        }
    }
}
