use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;

const TODO_FILE: &str = ".yangzz/todos.json";

/// TodoTool: manage a persistent task list
pub struct TodoTool;

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str { "todo" }

    fn description(&self) -> &str {
        "Manage a TODO/task list. Actions: list, add, done, remove. Tasks persist in .yangzz/todos.json."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "add", "done", "remove"],
                    "description": "Action to perform"
                },
                "task": {
                    "type": "string",
                    "description": "Task description (for add) or task number (for done/remove)"
                },
                "priority": {
                    "type": "string",
                    "enum": ["high", "medium", "low"],
                    "description": "Task priority (for add, default: medium)"
                }
            },
            "required": ["action"]
        })
    }

    fn is_read_only(&self) -> bool { false }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let action = input["action"].as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'action'".into()))?;

        let todo_path = ctx.cwd.join(TODO_FILE);

        // Ensure directory exists
        if let Some(parent) = todo_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        let mut todos = load_todos(&todo_path).await;

        match action {
            "list" => {
                if todos.is_empty() {
                    return Ok(ToolOutput::success("No tasks."));
                }
                let mut out = String::new();
                for (i, t) in todos.iter().enumerate() {
                    let status = if t.done { "✓" } else { "○" };
                    let pri = match t.priority.as_str() {
                        "high" => "🔴",
                        "low" => "🟢",
                        _ => "🟡",
                    };
                    out.push_str(&format!("{} {} [{}] {}\n", i + 1, status, pri, t.text));
                }
                Ok(ToolOutput::success(out))
            }
            "add" => {
                let task = input["task"].as_str()
                    .ok_or_else(|| ToolError::Validation("Missing 'task' for add".into()))?;
                let priority = input["priority"].as_str().unwrap_or("medium").to_string();
                todos.push(TodoItem { text: task.to_string(), done: false, priority });
                save_todos(&todo_path, &todos).await?;
                Ok(ToolOutput::success(format!("Added task #{}: {task}", todos.len())))
            }
            "done" => {
                let idx = parse_task_index(input, todos.len())?;
                todos[idx].done = true;
                save_todos(&todo_path, &todos).await?;
                Ok(ToolOutput::success(format!("Marked #{} as done: {}", idx + 1, todos[idx].text)))
            }
            "remove" => {
                let idx = parse_task_index(input, todos.len())?;
                let removed = todos.remove(idx);
                save_todos(&todo_path, &todos).await?;
                Ok(ToolOutput::success(format!("Removed #{}: {}", idx + 1, removed.text)))
            }
            _ => Err(ToolError::Validation(format!("Unknown action: {action}"))),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct TodoItem {
    text: String,
    done: bool,
    priority: String,
}

async fn load_todos(path: &PathBuf) -> Vec<TodoItem> {
    match tokio::fs::read_to_string(path).await {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

async fn save_todos(path: &PathBuf, todos: &[TodoItem]) -> Result<(), ToolError> {
    let json = serde_json::to_string_pretty(todos)
        .map_err(|e| ToolError::Execution(format!("JSON error: {e}")))?;
    tokio::fs::write(path, json).await
        .map_err(|e| ToolError::Execution(format!("Write error: {e}")))
}

fn parse_task_index(input: &Value, total: usize) -> Result<usize, ToolError> {
    let task = input["task"].as_str()
        .ok_or_else(|| ToolError::Validation("Missing 'task' number".into()))?;
    let idx: usize = task.trim().parse::<usize>()
        .map_err(|_| ToolError::Validation("'task' must be a number for done/remove".into()))?;
    if idx == 0 || idx > total {
        return Err(ToolError::Validation(format!("Task #{idx} not found (total: {total})")));
    }
    Ok(idx - 1)
}
