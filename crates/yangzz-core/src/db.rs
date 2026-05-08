use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};

/// Central SQLite database for yangzz — stores sessions, messages, and memories.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open (or create) the database at the given path.
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// In-memory database for tests.
    #[cfg(test)]
    pub fn open_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// Default database path: `~/.yangzz/yangzz.db`
    pub fn default_path() -> PathBuf {
        crate::paths::yangzz_dir().join("yangzz.db")
    }

    fn migrate(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL DEFAULT '',
                model TEXT NOT NULL DEFAULT '',
                provider TEXT NOT NULL DEFAULT '',
                cwd TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                role TEXT NOT NULL,
                content_json TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, id);

            CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL DEFAULT 'fact',
                content TEXT NOT NULL,
                project TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                content,
                kind,
                project,
                content='memories',
                content_rowid='id'
            );

            CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
                INSERT INTO memories_fts(rowid, content, kind, project)
                VALUES (new.id, new.content, new.kind, new.project);
            END;

            CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, content, kind, project)
                VALUES ('delete', old.id, old.content, old.kind, old.project);
            END;

            CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, content, kind, project)
                VALUES ('delete', old.id, old.content, old.kind, old.project);
                INSERT INTO memories_fts(rowid, content, kind, project)
                VALUES (new.id, new.content, new.kind, new.project);
            END;
            ",
        )?;
        Ok(())
    }

    // ── Sessions ──

    pub fn create_session(&self, id: &str, model: &str, provider: &str, cwd: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO sessions (id, model, provider, cwd) VALUES (?1, ?2, ?3, ?4)",
            params![id, model, provider, cwd],
        )?;
        Ok(())
    }

    pub fn update_session_title(&self, id: &str, title: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE sessions SET title = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![title, id],
        )?;
        Ok(())
    }

    pub fn list_sessions(&self, limit: usize) -> Result<Vec<SessionRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, model, provider, cwd, created_at, updated_at
             FROM sessions ORDER BY updated_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(SessionRow {
                id: row.get(0)?,
                title: row.get(1)?,
                model: row.get(2)?,
                provider: row.get(3)?,
                cwd: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    // ── Messages ──

    pub fn insert_message(&self, session_id: &str, role: &str, content_json: &str) -> Result<i64, rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO messages (session_id, role, content_json) VALUES (?1, ?2, ?3)",
            params![session_id, role, content_json],
        )?;
        self.conn.execute(
            "UPDATE sessions SET updated_at = datetime('now') WHERE id = ?1",
            params![session_id],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_messages(&self, session_id: &str) -> Result<Vec<MessageRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, role, content_json, created_at FROM messages WHERE session_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            Ok(MessageRow {
                id: row.get(0)?,
                role: row.get(1)?,
                content_json: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    // ── Memories (FTS5) ──

    pub fn insert_memory(&self, kind: &str, content: &str, project: &str) -> Result<i64, rusqlite::Error> {
        // Dedup: skip if exact content already exists for this project
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM memories WHERE content = ?1 AND project = ?2)",
            params![content, project],
            |row| row.get(0),
        )?;
        if exists {
            return Ok(0);
        }

        self.conn.execute(
            "INSERT INTO memories (kind, content, project) VALUES (?1, ?2, ?3)",
            params![kind, content, project],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn search_memories(&self, query: &str, project: &str, limit: usize) -> Result<Vec<MemoryRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.kind, m.content, m.project, m.created_at
             FROM memories_fts f
             JOIN memories m ON m.id = f.rowid
             WHERE memories_fts MATCH ?1
               AND (m.project = ?2 OR m.project = '')
             ORDER BY rank
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![query, project, limit], |row| {
            Ok(MemoryRow {
                id: row.get(0)?,
                kind: row.get(1)?,
                content: row.get(2)?,
                project: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn list_memories(&self, project: &str, limit: usize) -> Result<Vec<MemoryRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, content, project, created_at
             FROM memories
             WHERE project = ?1 OR project = ''
             ORDER BY updated_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![project, limit], |row| {
            Ok(MemoryRow {
                id: row.get(0)?,
                kind: row.get(1)?,
                content: row.get(2)?,
                project: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn delete_memory(&self, id: i64) -> Result<(), rusqlite::Error> {
        self.conn.execute("DELETE FROM memories WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Migrate entries from MEMORY.md into the database
    pub fn migrate_from_markdown(&self, content: &str, project: &str) -> Result<usize, rusqlite::Error> {
        let mut count = 0;
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(entry) = trimmed.strip_prefix("- ") {
                if entry.is_empty() {
                    continue;
                }
                // Parse kind tag: [pref], [scar], [fact], [ok]
                let (kind, text) = if let Some(rest) = entry.strip_prefix("[pref] ") {
                    ("pref", rest)
                } else if let Some(rest) = entry.strip_prefix("[scar] ") {
                    ("scar", rest)
                } else if let Some(rest) = entry.strip_prefix("[fact] ") {
                    ("fact", rest)
                } else if let Some(rest) = entry.strip_prefix("[ok] ") {
                    ("ok", rest)
                } else {
                    ("fact", entry)
                };
                if self.insert_memory(kind, text, project)? > 0 {
                    count += 1;
                }
            }
        }
        Ok(count)
    }
}

// ── Row types ──

#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: String,
    pub title: String,
    pub model: String,
    pub provider: String,
    pub cwd: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct MessageRow {
    pub id: i64,
    pub role: String,
    pub content_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct MemoryRow {
    pub id: i64,
    pub kind: String,
    pub content: String,
    pub project: String,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_crud() {
        let db = Database::open_memory().unwrap();
        let id = db.insert_memory("pref", "User prefers Rust", "myproject").unwrap();
        assert!(id > 0);

        // Dedup
        let id2 = db.insert_memory("pref", "User prefers Rust", "myproject").unwrap();
        assert_eq!(id2, 0);

        let results = db.search_memories("Rust", "myproject", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "User prefers Rust");

        db.delete_memory(id).unwrap();
        let results = db.search_memories("Rust", "myproject", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_session_messages() {
        let db = Database::open_memory().unwrap();
        db.create_session("s1", "gpt-4o", "openai", "/tmp").unwrap();
        db.insert_message("s1", "user", r#"[{"type":"text","text":"hello"}]"#).unwrap();
        db.insert_message("s1", "assistant", r#"[{"type":"text","text":"hi"}]"#).unwrap();

        let msgs = db.get_messages("s1").unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "user");

        let sessions = db.list_sessions(10).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "s1");
    }

    #[test]
    fn test_migrate_markdown() {
        let db = Database::open_memory().unwrap();
        let md = "# Memory\n\n- Project uses Rust\n- [pref] User prefers Chinese\n- [scar] Don't use unwrap in prod\n";
        let count = db.migrate_from_markdown(md, "test").unwrap();
        assert_eq!(count, 3);

        let all = db.list_memories("test", 10).unwrap();
        assert_eq!(all.len(), 3);
    }
}
