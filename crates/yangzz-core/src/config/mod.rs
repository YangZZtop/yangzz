mod presets;
pub mod model_meta;
pub mod settings;

pub use presets::{ProviderPreset, PRESETS};
pub use settings::Settings;

use crate::provider::{AnthropicProvider, ApiFormat, OpenAiCompatProvider, Provider, ProviderError};
use std::sync::Arc;

/// A provider with its name and API key, ready to be queried for models
pub struct AvailableProvider {
    pub name: String,
    pub provider: Arc<dyn Provider>,
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
            "anthropic" => {
                AnthropicProvider::with_base_url(&extra.api_key, &extra.base_url, extra.default_model.clone())
                    .ok().map(|p| Arc::new(p) as Arc<dyn Provider>)
            }
            _ => {
                OpenAiCompatProvider::new(&extra.name, &extra.api_key, &extra.base_url, extra.default_model.clone())
                    .ok().map(|p| Arc::new(p) as Arc<dyn Provider>)
            }
        };
        if let Some(p) = provider {
            seen.insert(extra.name.clone());
            result.push(AvailableProvider {
                name: extra.name.clone(),
                provider: p,
            });
        }
    }

    // Check each preset for its env var (add others that aren't the current)
    for preset in PRESETS {
        if preset.api_key_env.is_empty() {
            continue;
        }
        if seen.contains(preset.name) {
            continue;
        }
        let key = std::env::var(preset.api_key_env).unwrap_or_default();
        if key.is_empty() {
            continue;
        }
        let provider: Option<Arc<dyn Provider>> = match preset.api_format {
            ApiFormat::Anthropic => {
                AnthropicProvider::new(&key, None).ok().map(|p| Arc::new(p) as Arc<dyn Provider>)
            }
            ApiFormat::OpenAi | ApiFormat::Gemini => {
                OpenAiCompatProvider::new(preset.name, &key, preset.base_url, None)
                    .ok()
                    .map(|p| Arc::new(p) as Arc<dyn Provider>)
            }
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

/// Resolve a Provider from settings — the "轻配置" magic
///
/// Priority:
///   1. CLI args (provider, model, base_url)
///   2. Environment variables (YANGZZ_API_KEY, YANGZZ_MODEL, etc.)
///   3. Vendor-specific env vars (ANTHROPIC_API_KEY, OPENAI_API_KEY, etc.)
///   4. Config file settings
///   5. Built-in presets
pub fn resolve_provider(settings: &Settings) -> Result<Arc<dyn Provider>, ProviderError> {
    // First: try to match provider name against [[providers]] in config
    if let Some(ref provider_name) = settings.provider {
        for extra in &settings.providers {
            if extra.name.eq_ignore_ascii_case(provider_name) {
                let fmt = extra.api_format.as_deref().unwrap_or("openai");
                let model = settings.model.clone().or_else(|| extra.default_model.clone());
                return match fmt {
                    "anthropic" => {
                        let p = AnthropicProvider::with_base_url(&extra.api_key, &extra.base_url, model)?;
                        Ok(Arc::new(p) as Arc<dyn Provider>)
                    }
                    _ => {
                        let p = OpenAiCompatProvider::new(&extra.name, &extra.api_key, &extra.base_url, model)?;
                        Ok(Arc::new(p) as Arc<dyn Provider>)
                    }
                };
            }
        }
    }

    // Fallback: env vars / presets (original logic)
    let api_key = settings.api_key.as_deref().unwrap_or("");

    // If no explicit key but we have providers in config, use the first one
    if api_key.is_empty() {
        if let Some(first) = settings.providers.first() {
            let fmt = first.api_format.as_deref().unwrap_or("openai");
            let model = settings.model.clone().or_else(|| first.default_model.clone());
            return match fmt {
                "anthropic" => {
                    let p = AnthropicProvider::with_base_url(&first.api_key, &first.base_url, model)?;
                    Ok(Arc::new(p) as Arc<dyn Provider>)
                }
                _ => {
                    let p = OpenAiCompatProvider::new(&first.name, &first.api_key, &first.base_url, model)?;
                    Ok(Arc::new(p) as Arc<dyn Provider>)
                }
            };
        }
        return Err(ProviderError::Auth(
            "No API key found. Set YANGZZ_API_KEY or configure [[providers]] in config.toml.".into()
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
