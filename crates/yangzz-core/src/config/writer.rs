//! Programmatic config.toml editor — used by `/provider add|edit|remove|rename`
//! and `/key`, so users never have to open the file manually.
//!
//! Strategy: load → mutate in-memory struct → serialize back. This may not
//! preserve user comments / field ordering perfectly, but it's reliable and
//! all official yangzz config lines have well-known meanings (users rarely
//! add freeform comments beyond what our wizard writes).

use super::settings::{ExtraProvider, Settings};
use std::path::PathBuf;

/// Load the global config.toml into a Settings struct (without env/CLI overlays).
/// Returns Settings::default() if the file doesn't exist.
pub fn load_raw_config() -> Settings {
    let path = crate::paths::config_path();
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(s) = toml::from_str::<Settings>(&content) {
            return s;
        }
    }
    Settings::default()
}

/// Write Settings back to the global config.toml.
pub fn save_raw_config(settings: &Settings) -> Result<PathBuf, String> {
    crate::paths::ensure_yangzz_dir();
    let path = crate::paths::config_path();
    let toml_str = serialize_settings(settings);
    std::fs::write(&path, toml_str).map_err(|e| format!("write failed: {e}"))?;
    Ok(path)
}

/// Serialize Settings to TOML. We build it by hand to keep a clean layout
/// that preserves the feel of the wizard-generated config.
fn serialize_settings(s: &Settings) -> String {
    let mut out = String::new();

    if let Some(ref p) = s.provider {
        out.push_str(&format!("provider = {}\n", toml_str(p)));
    }
    if let Some(ref m) = s.model {
        out.push_str(&format!("model = {}\n", toml_str(m)));
    }
    if let Some(mt) = s.max_tokens {
        out.push_str(&format!("max_tokens = {mt}\n"));
    }
    if let Some(ref t) = s.temperature {
        out.push_str(&format!("temperature = {t}\n"));
    }
    if let Some(tb) = s.thinking_budget {
        out.push_str(&format!("thinking_budget = {tb}\n"));
    }
    if let Some(cw) = s.context_window {
        out.push_str(&format!("context_window = {cw}\n"));
    }
    if let Some(ref re) = s.reasoning_effort {
        out.push_str(&format!("reasoning_effort = {}\n", toml_str(re)));
    }

    for p in &s.providers {
        out.push_str("\n[[providers]]\n");
        out.push_str(&format!("name = {}\n", toml_str(&p.name)));
        out.push_str(&format!("api_key = {}\n", toml_str(&p.api_key)));
        out.push_str(&format!("base_url = {}\n", toml_str(&p.base_url)));
        if let Some(ref dm) = p.default_model {
            out.push_str(&format!("default_model = {}\n", toml_str(dm)));
        }
        if let Some(ref af) = p.api_format {
            out.push_str(&format!("api_format = {}\n", toml_str(af)));
        }
        if let Some(tb) = p.thinking_budget {
            out.push_str(&format!("thinking_budget = {tb}\n"));
        }
        if let Some(cw) = p.context_window {
            out.push_str(&format!("context_window = {cw}\n"));
        }
        if let Some(ref re) = p.reasoning_effort {
            out.push_str(&format!("reasoning_effort = {}\n", toml_str(re)));
        }
        if let Some(mt) = p.max_tokens {
            out.push_str(&format!("max_tokens = {mt}\n"));
        }
        if let Some(t) = p.temperature {
            out.push_str(&format!("temperature = {t}\n"));
        }
    }

    out
}

/// TOML-escape a string value.
fn toml_str(s: &str) -> String {
    // Prefer basic strings; only escape " and \ and control chars.
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ── High-level operations used by REPL commands ──

/// Add a new provider. Returns Err if a provider with that name already exists.
pub fn add_provider(provider: ExtraProvider) -> Result<PathBuf, String> {
    let mut s = load_raw_config();
    if s.providers
        .iter()
        .any(|p| p.name.eq_ignore_ascii_case(&provider.name))
    {
        return Err(format!("Provider '{}' already exists", provider.name));
    }
    // If this is the first provider ever, also set it as default.
    let is_first = s.providers.is_empty();
    s.providers.push(provider.clone());
    if is_first && s.provider.is_none() {
        s.provider = Some(provider.name.clone());
        if s.model.is_none() {
            s.model = provider.default_model.clone();
        }
    }
    save_raw_config(&s)
}

/// Remove a provider by name. Clears `provider`/`model` top-level if it was
/// pointing at the removed one.
pub fn remove_provider(name: &str) -> Result<PathBuf, String> {
    let mut s = load_raw_config();
    let before = s.providers.len();
    s.providers.retain(|p| !p.name.eq_ignore_ascii_case(name));
    if s.providers.len() == before {
        return Err(format!("Provider '{name}' not found"));
    }
    if s.provider
        .as_deref()
        .map(|n| n.eq_ignore_ascii_case(name))
        .unwrap_or(false)
    {
        s.provider = s.providers.first().map(|p| p.name.clone());
        s.model = s.providers.first().and_then(|p| p.default_model.clone());
    }
    save_raw_config(&s)
}

/// Rename a provider. Updates top-level `provider` if it referenced the old name.
pub fn rename_provider(old: &str, new: &str) -> Result<PathBuf, String> {
    if new.trim().is_empty() {
        return Err("New name cannot be empty".into());
    }
    let mut s = load_raw_config();
    if s.providers.iter().any(|p| p.name.eq_ignore_ascii_case(new)) {
        return Err(format!("Name '{new}' already taken"));
    }
    let mut found = false;
    for p in &mut s.providers {
        if p.name.eq_ignore_ascii_case(old) {
            p.name = new.to_string();
            found = true;
        }
    }
    if !found {
        return Err(format!("Provider '{old}' not found"));
    }
    if s.provider
        .as_deref()
        .map(|n| n.eq_ignore_ascii_case(old))
        .unwrap_or(false)
    {
        s.provider = Some(new.to_string());
    }
    save_raw_config(&s)
}

/// Update one field on a provider. `field` is one of: api_key, base_url,
/// default_model, api_format, max_tokens, thinking_budget, context_window,
/// reasoning_effort, temperature.
pub fn edit_provider_field(name: &str, field: &str, value: &str) -> Result<PathBuf, String> {
    let mut s = load_raw_config();
    let p = s
        .providers
        .iter_mut()
        .find(|p| p.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("Provider '{name}' not found"))?;

    match field {
        "api_key" => p.api_key = value.to_string(),
        "base_url" => p.base_url = value.to_string(),
        "default_model" => p.default_model = Some(value.to_string()),
        "api_format" => p.api_format = Some(value.to_string()),
        "reasoning_effort" => p.reasoning_effort = Some(value.to_string()),
        "max_tokens" => {
            p.max_tokens = Some(value.parse().map_err(|_| "max_tokens must be a number")?);
        }
        "thinking_budget" => {
            p.thinking_budget = Some(
                value
                    .parse()
                    .map_err(|_| "thinking_budget must be a number")?,
            );
        }
        "context_window" => {
            p.context_window = Some(
                value
                    .parse()
                    .map_err(|_| "context_window must be a number")?,
            );
        }
        "temperature" => {
            p.temperature = Some(value.parse().map_err(|_| "temperature must be a float")?);
        }
        other => return Err(format!("Unknown field '{other}'")),
    }

    save_raw_config(&s)
}

/// Rotate just the api_key for a provider — shortcut for `/key`.
pub fn rotate_api_key(name: &str, new_key: &str) -> Result<PathBuf, String> {
    edit_provider_field(name, "api_key", new_key)
}
