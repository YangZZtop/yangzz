use super::Tool;
use crate::provider::ToolDefinition;
use std::collections::HashMap;
use std::sync::Arc;

/// Registry of all available tools
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register all built-in tools
    pub fn with_builtins(_cwd: &std::path::Path) -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(super::builtin::BashTool));
        registry.register(Arc::new(super::builtin::FileReadTool));
        registry.register(Arc::new(super::builtin::FileEditTool));
        registry.register(Arc::new(super::builtin::FileWriteTool));
        registry.register(Arc::new(super::builtin::FileAppendTool));
        registry.register(Arc::new(super::builtin::MultiEditTool));
        registry.register(Arc::new(super::builtin::GrepTool));
        registry.register(Arc::new(super::builtin::GlobTool));
        registry.register(Arc::new(super::builtin::ListDirTool));
        registry.register(Arc::new(super::builtin::TreeTool));
        registry.register(Arc::new(super::builtin::FetchTool));
        registry.register(Arc::new(super::builtin::AskUserTool));
        registry.register(Arc::new(super::builtin::NotebookReadTool));
        registry.register(Arc::new(super::builtin::NotebookEditTool));
        registry.register(Arc::new(super::builtin::SubAgentTool));
        registry.register(Arc::new(super::builtin::TodoTool));
        registry.register(Arc::new(super::builtin::ParallelEditTool));
        registry
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    /// Generate tool definitions for the API
    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|t| ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
            })
            .collect()
    }

    pub fn names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
