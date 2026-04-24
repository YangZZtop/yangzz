use crate::message::Message;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// A conversation session that can be saved/restored
#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub model: String,
    pub provider: String,
    pub cwd: String,
    pub messages: Vec<Message>,
}

impl Session {
    pub fn new(model: &str, provider: &str) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: Uuid::new_v4().to_string(),
            created_at: now.clone(),
            updated_at: now,
            model: model.to_string(),
            provider: provider.to_string(),
            cwd: std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            messages: Vec::new(),
        }
    }

    /// Session storage directory: `~/.yangzz/sessions/`
    fn storage_dir() -> PathBuf {
        let dir = crate::paths::sessions_dir();
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    /// Save session to disk
    pub fn save(&mut self) -> anyhow::Result<()> {
        self.updated_at = Utc::now().to_rfc3339();
        let path = Self::storage_dir().join(format!("{}.json", self.id));
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Load the most recent session for the current directory
    pub fn load_latest() -> Option<Self> {
        let cwd = std::env::current_dir().ok()?.to_string_lossy().to_string();
        let dir = Self::storage_dir();

        let mut sessions: Vec<(String, Self)> = std::fs::read_dir(&dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .filter_map(|e| {
                let content = std::fs::read_to_string(e.path()).ok()?;
                let session: Session = serde_json::from_str(&content).ok()?;
                Some((session.updated_at.clone(), session))
            })
            .filter(|(_, s)| s.cwd == cwd)
            .collect();

        sessions.sort_by(|a, b| b.0.cmp(&a.0));
        sessions.into_iter().next().map(|(_, s)| s)
    }

    /// Load a specific session by ID
    pub fn load(id: &str) -> Option<Self> {
        let path = Self::storage_dir().join(format!("{id}.json"));
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Search across all sessions for a keyword (for /recall)
    pub fn search(query: &str) -> Vec<SearchResult> {
        let dir = Self::storage_dir();
        let lower_query = query.to_lowercase();
        let mut results = Vec::new();

        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return results,
        };

        for entry in entries.filter_map(|e| e.ok()) {
            if !entry.path().extension().is_some_and(|ext| ext == "json") {
                continue;
            }
            let content = match std::fs::read_to_string(entry.path()) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let session: Session = match serde_json::from_str(&content) {
                Ok(s) => s,
                Err(_) => continue,
            };

            for msg in &session.messages {
                for block in &msg.content {
                    let text = match block {
                        crate::message::ContentBlock::Text { text } => text.as_str(),
                        _ => continue,
                    };
                    if text.to_lowercase().contains(&lower_query) {
                        // Extract a snippet around the match — all indices
                        // floored/ceiled to char boundaries so CJK/emoji
                        // don't cause UTF-8 slice panics.
                        let idx = text.to_lowercase().find(&lower_query).unwrap_or(0);
                        let mut start = idx.saturating_sub(60);
                        while start > 0 && !text.is_char_boundary(start) {
                            start -= 1;
                        }
                        let mut end = (idx + query.len() + 60).min(text.len());
                        while end < text.len() && !text.is_char_boundary(end) {
                            end += 1;
                        }
                        let snippet = &text[start..end];

                        results.push(SearchResult {
                            session_id: session.id.clone(),
                            date: session.updated_at.clone(),
                            model: session.model.clone(),
                            snippet: snippet.to_string(),
                        });

                        if results.len() >= 20 {
                            return results;
                        }
                        break; // one match per message is enough
                    }
                }
            }
        }
        results
    }
}

/// A search hit across sessions
#[derive(Debug)]
pub struct SearchResult {
    pub session_id: String,
    pub date: String,
    pub model: String,
    pub snippet: String,
}
