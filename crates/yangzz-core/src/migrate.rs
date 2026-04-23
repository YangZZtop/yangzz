//! Configuration migration: import settings from Claude Code / Codex CLI / Cursor

use std::path::{Path, PathBuf};
use tracing::info;

/// Detected source tool
#[derive(Debug, Clone)]
pub enum SourceTool {
    ClaudeCode,
    CodexCli,
    Cursor,
    CopilotChat,
}

impl SourceTool {
    pub fn name(&self) -> &str {
        match self {
            SourceTool::ClaudeCode => "Claude Code",
            SourceTool::CodexCli => "Codex CLI",
            SourceTool::Cursor => "Cursor",
            SourceTool::CopilotChat => "Copilot Chat",
        }
    }
}

/// Detected migration source
#[derive(Debug)]
pub struct MigrationSource {
    pub tool: SourceTool,
    pub config_path: PathBuf,
    pub items: Vec<MigrationItem>,
}

#[derive(Debug)]
pub struct MigrationItem {
    pub key: String,
    pub value: String,
    pub description: String,
}

/// Detect available migration sources
pub fn detect_sources() -> Vec<MigrationSource> {
    let home = dirs::home_dir().unwrap_or_default();
    let mut sources = Vec::new();

    // Claude Code: ~/.claude/
    let claude_dir = home.join(".claude");
    if claude_dir.exists() {
        let mut items = Vec::new();

        // Check for CLAUDE.md (system instructions)
        let claude_md = claude_dir.join("CLAUDE.md");
        if claude_md.exists() {
            if let Ok(content) = std::fs::read_to_string(&claude_md) {
                items.push(MigrationItem {
                    key: "system_instructions".into(),
                    value: content,
                    description: "CLAUDE.md system instructions → MEMORY.md".into(),
                });
            }
        }

        // Check for settings.json
        let settings = claude_dir.join("settings.json");
        if settings.exists() {
            if let Ok(content) = std::fs::read_to_string(&settings) {
                items.push(MigrationItem {
                    key: "settings".into(),
                    value: content,
                    description: "Claude Code settings".into(),
                });
            }
        }

        if !items.is_empty() {
            sources.push(MigrationSource {
                tool: SourceTool::ClaudeCode,
                config_path: claude_dir,
                items,
            });
        }
    }

    // Codex CLI: ~/.codex/
    let codex_dir = home.join(".codex");
    if codex_dir.exists() {
        let mut items = Vec::new();

        let instructions = codex_dir.join("instructions.md");
        if instructions.exists() {
            if let Ok(content) = std::fs::read_to_string(&instructions) {
                items.push(MigrationItem {
                    key: "instructions".into(),
                    value: content,
                    description: "Codex instructions.md → MEMORY.md".into(),
                });
            }
        }

        let config = codex_dir.join("config.yaml");
        if config.exists() {
            if let Ok(content) = std::fs::read_to_string(&config) {
                items.push(MigrationItem {
                    key: "config".into(),
                    value: content,
                    description: "Codex CLI config.yaml".into(),
                });
            }
        }

        if !items.is_empty() {
            sources.push(MigrationSource {
                tool: SourceTool::CodexCli,
                config_path: codex_dir,
                items,
            });
        }
    }

    // Cursor: ~/.cursor/ or project .cursorrules
    let cursor_dir = home.join(".cursor");
    if cursor_dir.exists() {
        let mut items = Vec::new();

        let rules = cursor_dir.join("rules");
        if rules.exists() {
            if let Ok(entries) = std::fs::read_dir(&rules) {
                for entry in entries.filter_map(|e| e.ok()) {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        items.push(MigrationItem {
                            key: format!("rule:{}", entry.file_name().to_string_lossy()),
                            value: content,
                            description: format!("Cursor rule: {}", entry.file_name().to_string_lossy()),
                        });
                    }
                }
            }
        }

        if !items.is_empty() {
            sources.push(MigrationSource {
                tool: SourceTool::Cursor,
                config_path: cursor_dir,
                items,
            });
        }
    }

    // Project-level .cursorrules
    let cwd = std::env::current_dir().unwrap_or_default();
    let cursorrules = cwd.join(".cursorrules");
    if cursorrules.exists() {
        if let Ok(content) = std::fs::read_to_string(&cursorrules) {
            sources.push(MigrationSource {
                tool: SourceTool::Cursor,
                config_path: cursorrules,
                items: vec![MigrationItem {
                    key: "cursorrules".into(),
                    value: content,
                    description: "Project .cursorrules → MEMORY.md".into(),
                }],
            });
        }
    }

    sources
}

/// Migrate: convert detected sources into MEMORY.md entries
pub fn migrate_to_memory(sources: &[MigrationSource], cwd: &Path) -> Result<Vec<String>, String> {
    let mut migrated = Vec::new();

    for source in sources {
        for item in &source.items {
            match item.key.as_str() {
                "system_instructions" | "instructions" | "cursorrules" => {
                    // These are instruction files → append to MEMORY.md
                    let entry = format!(
                        "[Migrated from {}] {}",
                        source.tool.name(),
                        item.value.lines().take(20).collect::<Vec<_>>().join("\n")
                    );
                    crate::memory::append_memory(cwd, &entry)?;
                    migrated.push(format!("✓ {} → MEMORY.md", item.description));
                }
                _ if item.key.starts_with("rule:") => {
                    let entry = format!(
                        "[Cursor rule: {}] {}",
                        item.key.strip_prefix("rule:").unwrap_or(&item.key),
                        item.value.lines().take(10).collect::<Vec<_>>().join("\n")
                    );
                    crate::memory::append_memory(cwd, &entry)?;
                    migrated.push(format!("✓ {} → MEMORY.md", item.description));
                }
                _ => {
                    migrated.push(format!("⊘ Skipped: {} (manual migration needed)", item.description));
                }
            }
        }
    }

    info!("Migration complete: {} items processed", migrated.len());
    Ok(migrated)
}
