use dialoguer::{theme::ColorfulTheme, Input, Select};
use super::format::*;
use super::i18n::t;

/// A group of models from one provider
pub struct ProviderModels {
    pub provider_name: String,
    pub models: Vec<String>,
}

/// Cross-provider model selector.
///
/// `provider_models`: models grouped by provider (from all available providers).
/// Returns (model, provider_name) if selected.
pub fn select_model(
    current_model: &str,
    current_provider: &str,
    provider_models: &[ProviderModels],
) -> Option<(String, String)> {
    println!();
    let s = t();
    println!("  {BOLD_GOLD}{}{RESET}", s.switch_model);
    println!("  {DIM}{}: {BOLD}{current_model}{RESET}{DIM} via {current_provider}{RESET}", s.current_label);
    println!();

    // (display_string, model_id, provider_name)
    let mut items: Vec<String> = Vec::new();
    let mut entries: Vec<(String, String)> = Vec::new(); // (model, provider)
    let mut current_idx: Option<usize> = None;

    for group in provider_models {
        if group.models.is_empty() {
            continue;
        }
        // Section header
        let header = format!("  {BOLD_GOLD}── {} ──{RESET}", group.provider_name);
        items.push(header);
        entries.push(("__header__".to_string(), group.provider_name.clone()));

        for m in &group.models {
            let is_cur = m == current_model && group.provider_name == current_provider;
            let mark = if is_cur { format!(" {GREEN}✓{RESET}") } else { String::new() };
            items.push(format!("    {m}{mark}"));
            if is_cur {
                current_idx = Some(entries.len());
            }
            entries.push((m.clone(), group.provider_name.clone()));
        }
    }

    // If current model not in any list, prepend it
    if current_idx.is_none() && !current_model.is_empty() {
        let line = format!("    {current_model} {GREEN}✓{RESET} {DIM}({0}){RESET}", s.current_label);
        items.insert(0, line);
        entries.insert(0, (current_model.to_string(), current_provider.to_string()));
        current_idx = Some(0);
    }

    // Append custom input option
    items.push(format!("  {GOLD}{}{RESET}", s.custom_model));
    entries.push(("__custom__".to_string(), String::new()));

    let default_idx = current_idx.unwrap_or(0);

    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&items)
        .default(default_idx)
        .interact_opt()
        .ok()?;

    let idx = selection?;
    let (ref chosen_model, ref chosen_provider) = entries[idx];

    if chosen_model == "__header__" {
        // User selected a section header — ignore
        return None;
    }

    if chosen_model == "__custom__" {
        println!();
        let custom: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("  {}", s.custom_model_prompt))
            .interact_text()
            .ok()?;
        let custom = custom.trim().to_string();
        if custom.is_empty() {
            return None;
        }
        let provider = crate::repl::detect_provider_from_model(&custom)
            .unwrap_or(current_provider)
            .to_string();
        Some((custom, provider))
    } else {
        Some((chosen_model.clone(), chosen_provider.clone()))
    }
}

/// Show command picker (like Claude Code slash command autocomplete)
pub fn select_command(commands: &[(String, String)]) -> Option<String> {
    if commands.is_empty() {
        return None;
    }

    let items: Vec<String> = commands
        .iter()
        .map(|(cmd, desc)| format!("{:<20} {}", cmd, desc))
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&items)
        .default(0)
        .interact_opt()
        .ok()?;

    selection.map(|idx| commands[idx].0.clone())
}
