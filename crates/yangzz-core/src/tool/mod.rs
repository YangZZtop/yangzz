pub mod builtin;
mod executor;
mod registry;

pub use executor::ToolExecutor;
pub use registry::ToolRegistry;

use async_trait::async_trait;
use serde_json::Value;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

/// Tool execution context
pub struct ToolContext {
    pub cwd: std::path::PathBuf,
}

impl ToolContext {
    /// Resolve an existing file/directory path and ensure it stays inside the
    /// active workspace.
    pub fn resolve_existing_path(&self, raw_path: &str) -> Result<PathBuf, ToolError> {
        let candidate = self.join_path(raw_path);
        let resolved = candidate.canonicalize().map_err(|e| {
            ToolError::Execution(format!("Cannot resolve path {}: {e}", candidate.display()))
        })?;
        self.ensure_within_workspace(raw_path, &resolved)
    }

    /// Resolve a write target and ensure its nearest existing ancestor stays
    /// inside the active workspace.
    pub fn resolve_path_for_write(&self, raw_path: &str) -> Result<PathBuf, ToolError> {
        let candidate = self.join_path(raw_path);
        if candidate.exists() {
            let resolved = candidate.canonicalize().map_err(|e| {
                ToolError::Execution(format!("Cannot resolve path {}: {e}", candidate.display()))
            })?;
            return self.ensure_within_workspace(raw_path, &resolved);
        }

        let (ancestor, suffix) = nearest_existing_ancestor(&candidate).ok_or_else(|| {
            ToolError::Execution(format!("Cannot resolve parent for {}", candidate.display()))
        })?;
        let ancestor = ancestor.canonicalize().map_err(|e| {
            ToolError::Execution(format!("Cannot resolve path {}: {e}", ancestor.display()))
        })?;
        let mut resolved = self.ensure_within_workspace(raw_path, &ancestor)?;
        for segment in suffix {
            resolved.push(segment);
        }
        Ok(resolved)
    }

    fn join_path(&self, raw_path: &str) -> PathBuf {
        let path = Path::new(raw_path);
        let combined = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        };
        normalize_path(&combined)
    }

    fn ensure_within_workspace(
        &self,
        raw_path: &str,
        resolved: &Path,
    ) -> Result<PathBuf, ToolError> {
        let root = self.cwd.canonicalize().map_err(|e| {
            ToolError::Execution(format!(
                "Cannot resolve workspace root {}: {e}",
                self.cwd.display()
            ))
        })?;
        if resolved.starts_with(&root) {
            Ok(resolved.to_path_buf())
        } else {
            Err(ToolError::PermissionDenied(format!(
                "Path '{raw_path}' escapes workspace root {}",
                root.display()
            )))
        }
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut prefix: Option<OsString> = None;
    let mut has_root = false;
    let mut parts: Vec<OsString> = Vec::new();

    for component in path.components() {
        match component {
            Component::Prefix(value) => {
                prefix = Some(value.as_os_str().to_os_string());
            }
            Component::RootDir => {
                has_root = true;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.pop().is_none() && !has_root {
                    parts.push(OsString::from(".."));
                }
            }
            Component::Normal(value) => parts.push(value.to_os_string()),
        }
    }

    let mut normalized = PathBuf::new();
    if let Some(prefix) = prefix {
        normalized.push(prefix);
    }
    if has_root {
        normalized.push(std::path::MAIN_SEPARATOR.to_string());
    }
    for part in parts {
        normalized.push(part);
    }

    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

fn nearest_existing_ancestor(path: &Path) -> Option<(PathBuf, Vec<OsString>)> {
    let mut current = path.to_path_buf();
    let mut suffix = Vec::new();

    loop {
        if current.exists() {
            suffix.reverse();
            return Some((current, suffix));
        }

        let name = current.file_name()?.to_os_string();
        suffix.push(name);
        current = current.parent()?.to_path_buf();
    }
}

/// Tool output
#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub content: String,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
        }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
        }
    }
}

/// Tool errors
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Execution error: {0}")]
    Execution(String),

    #[error("Tool not found: {0}")]
    NotFound(String),
}

/// The Tool trait — one of the 5 核心元
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique tool name
    fn name(&self) -> &str;

    /// Human-readable description
    fn description(&self) -> &str;

    /// JSON Schema for input parameters
    fn input_schema(&self) -> Value;

    /// Is this a read-only operation?
    fn is_read_only(&self) -> bool {
        false
    }

    /// Could this destroy data?
    fn is_destructive(&self) -> bool {
        false
    }

    /// Execute the tool
    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError>;
}

#[cfg(test)]
mod tests {
    use super::ToolContext;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_test_dir(label: &str) -> PathBuf {
        let pid = std::process::id();

        for _ in 0..1024 {
            let seq = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("yangzz-tool-{label}-{pid}-{seq}"));
            match fs::create_dir(&dir) {
                Ok(_) => return dir,
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => panic!("failed to create test dir {}: {err}", dir.display()),
            }
        }

        panic!("failed to allocate unique test dir for {label}");
    }

    #[test]
    fn resolve_existing_path_rejects_absolute_path_outside_workspace() {
        let workspace = unique_test_dir("workspace");
        let outside = unique_test_dir("outside");
        let outside_file = outside.join("secret.txt");
        fs::write(&outside_file, "nope").unwrap();

        let ctx = ToolContext {
            cwd: workspace.clone(),
        };

        let err = ctx
            .resolve_existing_path(outside_file.to_str().unwrap())
            .unwrap_err();

        assert!(matches!(err, super::ToolError::PermissionDenied(_)));

        let _ = fs::remove_dir_all(&workspace);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn resolve_existing_path_allows_file_inside_workspace() {
        let workspace = unique_test_dir("workspace");
        let file = workspace.join("src").join("main.rs");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "fn main() {}").unwrap();

        let ctx = ToolContext {
            cwd: workspace.clone(),
        };

        let resolved = ctx.resolve_existing_path("src/main.rs").unwrap();
        assert_eq!(resolved, file.canonicalize().unwrap());

        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn resolve_path_for_write_rejects_new_file_outside_workspace() {
        let workspace = unique_test_dir("workspace");
        let outside = unique_test_dir("outside");
        let outside_target = outside.join("new.txt");

        let ctx = ToolContext {
            cwd: workspace.clone(),
        };

        let err = ctx
            .resolve_path_for_write(outside_target.to_str().unwrap())
            .unwrap_err();

        assert!(matches!(err, super::ToolError::PermissionDenied(_)));

        let _ = fs::remove_dir_all(&workspace);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn resolve_path_for_write_allows_nested_file_inside_workspace() {
        let workspace = unique_test_dir("workspace");
        let ctx = ToolContext {
            cwd: workspace.clone(),
        };

        let resolved = ctx.resolve_path_for_write("notes/daily/today.md").unwrap();

        assert_eq!(
            resolved,
            workspace
                .canonicalize()
                .unwrap()
                .join("notes/daily/today.md")
        );

        let _ = fs::remove_dir_all(&workspace);
    }
}
