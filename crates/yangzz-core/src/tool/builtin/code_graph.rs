use crate::codegraph::CodeGraph;
use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{Value, json};

pub struct CodeGraphTool;

#[async_trait]
impl Tool for CodeGraphTool {
    fn name(&self) -> &str {
        "code_graph"
    }

    fn description(&self) -> &str {
        "AST-level code understanding via tree-sitter. \
         Modes: 'find' (search symbol definitions by name), \
         'references' (files that mention a symbol), \
         'symbols' (list all symbols in a file), \
         'stats' (total files + symbols by kind). \
         Supports Rust (.rs), TypeScript (.ts/.tsx), Python (.py)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "mode": {
                    "type": "string",
                    "enum": ["find", "references", "symbols", "stats"],
                    "description": "Query kind."
                },
                "name": {
                    "type": "string",
                    "description": "Symbol name (for 'find' and 'references'). Substring match, case-insensitive for 'find'."
                },
                "file": {
                    "type": "string",
                    "description": "File path relative to cwd (for 'symbols')."
                },
                "path": {
                    "type": "string",
                    "description": "Directory to index (default: cwd)."
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results (default 50).",
                    "default": 50
                }
            },
            "required": ["mode"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let mode = input["mode"]
            .as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'mode' field".into()))?;

        let root = match input["path"].as_str() {
            Some(p) => ctx.resolve_existing_path(p)?,
            None => ctx.cwd.clone(),
        };
        let limit = input["limit"].as_u64().unwrap_or(50) as usize;

        let graph = CodeGraph::new(&root);

        match mode {
            "symbols" => {
                let file = input["file"]
                    .as_str()
                    .ok_or_else(|| ToolError::Validation("'symbols' mode needs 'file'".into()))?;
                let abs = ctx.resolve_existing_path(file)?;
                let syms = graph
                    .index_file(&abs)
                    .map_err(|e| ToolError::Execution(format!("parse failed: {e}")))?;
                if syms.is_empty() {
                    return Ok(ToolOutput::success(format!(
                        "No symbols found in {}",
                        abs.display()
                    )));
                }
                let mut lines = vec![format!("{} symbols in {}:", syms.len(), abs.display())];
                for s in syms.iter().take(limit) {
                    lines.push(format!("  {}:{}  {}  {}", display_rel(&s.file, &root), s.line, s.kind.as_str(), s.name));
                }
                Ok(ToolOutput::success(lines.join("\n")))
            }
            "find" => {
                let name = input["name"]
                    .as_str()
                    .ok_or_else(|| ToolError::Validation("'find' mode needs 'name'".into()))?;
                graph
                    .index_all()
                    .map_err(|e| ToolError::Execution(format!("index failed: {e}")))?;
                let mut hits = graph.find(name);
                hits.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
                if hits.is_empty() {
                    return Ok(ToolOutput::success(format!("No symbols matching '{name}'")));
                }
                let shown = hits.len().min(limit);
                let mut lines = vec![format!("{} matches for '{}' ({} shown):", hits.len(), name, shown)];
                for s in hits.iter().take(limit) {
                    lines.push(format!(
                        "  {}:{}  {}  {}",
                        display_rel(&s.file, &root),
                        s.line,
                        s.kind.as_str(),
                        s.name
                    ));
                }
                Ok(ToolOutput::success(lines.join("\n")))
            }
            "references" => {
                let name = input["name"]
                    .as_str()
                    .ok_or_else(|| ToolError::Validation("'references' mode needs 'name'".into()))?;
                let hits = graph
                    .find_references(name)
                    .map_err(|e| ToolError::Execution(format!("scan failed: {e}")))?;
                if hits.is_empty() {
                    return Ok(ToolOutput::success(format!(
                        "No files reference '{name}'"
                    )));
                }
                let shown = hits.len().min(limit);
                let mut lines = vec![format!(
                    "{} files reference '{}' ({} shown, text-level):",
                    hits.len(),
                    name,
                    shown
                )];
                for f in hits.iter().take(limit) {
                    lines.push(format!("  {}", display_rel(f, &root)));
                }
                Ok(ToolOutput::success(lines.join("\n")))
            }
            "stats" => {
                graph
                    .index_all()
                    .map_err(|e| ToolError::Execution(format!("index failed: {e}")))?;
                let s = graph.stats();
                let mut lines = vec![
                    format!("Files indexed: {}", s.files),
                    format!("Total symbols: {}", s.symbols),
                    "By kind:".to_string(),
                ];
                let mut pairs: Vec<_> = s.by_kind.iter().collect();
                pairs.sort_by(|a, b| b.1.cmp(a.1));
                for (kind, count) in pairs {
                    lines.push(format!("  {kind}: {count}"));
                }
                Ok(ToolOutput::success(lines.join("\n")))
            }
            other => Err(ToolError::Validation(format!(
                "Unknown mode '{other}' — expected find|references|symbols|stats"
            ))),
        }
    }
}

fn display_rel(file: &std::path::Path, root: &std::path::Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .display()
        .to_string()
}
