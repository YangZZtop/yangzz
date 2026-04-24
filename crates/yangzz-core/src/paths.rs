//! Unified path resolution for yangzz.
//!
//! All yangzz data (config, sessions, memory, cache) lives under a SINGLE
//! directory that varies by platform:
//!
//!   macOS   → ~/.yangzz/           (was: ~/Library/Application Support/yangzz/)
//!   Linux   → ~/.yangzz/           (was: ~/.config/yangzz/ + ~/.local/share/yangzz/)
//!   Windows → %USERPROFILE%\.yangzz\  (was: %APPDATA%\yangzz\)
//!
//! This makes uninstall trivial: `rm -rf ~/.yangzz` removes everything.
//!
//! Legacy locations are still read (for back-compat) via `legacy_*()` helpers.
//! The migration helper `maybe_migrate_legacy()` moves old data on first launch.

use std::path::PathBuf;

/// The ONE yangzz directory — `~/.yangzz/` on all platforms.
pub fn yangzz_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".yangzz")
}

/// Global config file: `~/.yangzz/config.toml`
pub fn config_path() -> PathBuf {
    yangzz_dir().join("config.toml")
}

/// Session storage directory: `~/.yangzz/sessions/`
pub fn sessions_dir() -> PathBuf {
    yangzz_dir().join("sessions")
}

/// Global MEMORY.md: `~/.yangzz/MEMORY.md`
pub fn memory_path() -> PathBuf {
    yangzz_dir().join("MEMORY.md")
}

/// Ensure `~/.yangzz/` exists. Best-effort; ignores errors.
pub fn ensure_yangzz_dir() {
    let _ = std::fs::create_dir_all(yangzz_dir());
}

// ── Legacy locations (pre-v0.3.0) — for migration ──

/// Legacy config file path (pre-0.3.0).
/// macOS: `~/Library/Application Support/yangzz/config.toml`
/// Linux: `~/.config/yangzz/config.toml`
/// Windows: `%APPDATA%\yangzz\config.toml`
pub fn legacy_config_path() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        dirs::data_dir().map(|d| d.join("yangzz").join("config.toml"))
    } else {
        dirs::config_dir().map(|d| d.join("yangzz").join("config.toml"))
    }
}

/// Legacy sessions dir (pre-0.3.0).
pub fn legacy_sessions_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("yangzz").join("sessions"))
}

/// Legacy MEMORY.md path (pre-0.3.0).
pub fn legacy_memory_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("yangzz").join("MEMORY.md"))
}

/// Migrate pre-0.3.0 data to `~/.yangzz/` on first launch.
///
/// Runs once. Copies config, sessions, MEMORY.md if found at legacy locations
/// and not yet present at new locations. Does NOT delete legacy (user can
/// clean up manually once they're sure it works).
///
/// Returns true if anything was migrated.
pub fn maybe_migrate_legacy() -> bool {
    ensure_yangzz_dir();
    let mut migrated = false;

    // Config
    if !config_path().exists() {
        if let Some(legacy) = legacy_config_path() {
            if legacy.exists() {
                if std::fs::copy(&legacy, config_path()).is_ok() {
                    migrated = true;
                }
            }
        }
    }

    // Sessions directory — copy all files
    if !sessions_dir().exists() {
        if let Some(legacy) = legacy_sessions_dir() {
            if legacy.exists() {
                let _ = std::fs::create_dir_all(sessions_dir());
                if let Ok(entries) = std::fs::read_dir(&legacy) {
                    for entry in entries.flatten() {
                        let src = entry.path();
                        if src.is_file() {
                            if let Some(name) = src.file_name() {
                                let dst = sessions_dir().join(name);
                                if std::fs::copy(&src, &dst).is_ok() {
                                    migrated = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // MEMORY.md
    if !memory_path().exists() {
        if let Some(legacy) = legacy_memory_path() {
            if legacy.exists() {
                if std::fs::copy(&legacy, memory_path()).is_ok() {
                    migrated = true;
                }
            }
        }
    }

    migrated
}

/// Human-readable report of all yangzz paths — used by `--where` and
/// `--uninstall`.
pub fn all_paths_report() -> Vec<(&'static str, PathBuf, bool)> {
    let mut out = Vec::new();
    let cfg = config_path();
    let sess = sessions_dir();
    let mem = memory_path();
    out.push(("config.toml", cfg.clone(), cfg.exists()));
    out.push(("sessions/", sess.clone(), sess.exists()));
    out.push(("MEMORY.md", mem.clone(), mem.exists()));
    out
}
