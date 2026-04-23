//! Task queue: schedule and manage background/foreground agent tasks
//! Supports shell, agent, and dream (autonomous) task types with priority queue.

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

/// Task types following nocode's model
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskType {
    /// Direct shell command execution
    Shell,
    /// Agent-driven task (uses agentic loop)
    Agent,
    /// Autonomous/dream mode (agent works independently)
    Dream,
}

impl TaskType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::Shell => "shell",
            TaskType::Agent => "agent",
            TaskType::Dream => "dream",
        }
    }
}

/// Task priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    High = 0,
    Normal = 1,
    Low = 2,
}

/// A queued task
#[derive(Debug, Clone)]
pub struct Task {
    pub id: usize,
    pub task_type: TaskType,
    pub description: String,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn emoji(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "⏳",
            TaskStatus::Running => "🔄",
            TaskStatus::Completed => "✅",
            TaskStatus::Failed => "❌",
            TaskStatus::Cancelled => "🚫",
        }
    }
}

/// Thread-safe task queue
pub struct TaskQueue {
    tasks: Arc<Mutex<VecDeque<Task>>>,
    next_id: Arc<Mutex<usize>>,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(VecDeque::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    /// Enqueue a new task
    pub async fn enqueue(&self, task_type: TaskType, description: &str, priority: TaskPriority) -> usize {
        let mut id_lock = self.next_id.lock().await;
        let id = *id_lock;
        *id_lock += 1;

        let task = Task {
            id,
            task_type,
            description: description.to_string(),
            priority,
            status: TaskStatus::Pending,
            result: None,
        };

        let mut tasks = self.tasks.lock().await;
        // Insert by priority
        let pos = tasks.iter().position(|t| t.priority > priority).unwrap_or(tasks.len());
        tasks.insert(pos, task);

        info!("Task #{id} enqueued: {description}");
        id
    }

    /// Dequeue the next pending task
    pub async fn dequeue(&self) -> Option<Task> {
        let mut tasks = self.tasks.lock().await;
        let pos = tasks.iter().position(|t| t.status == TaskStatus::Pending)?;
        let task = tasks.get_mut(pos)?;
        task.status = TaskStatus::Running;
        Some(task.clone())
    }

    /// Mark a task as completed
    pub async fn complete(&self, id: usize, result: String) {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.iter_mut().find(|t| t.id == id) {
            task.status = TaskStatus::Completed;
            task.result = Some(result);
        }
    }

    /// Mark a task as failed
    pub async fn fail(&self, id: usize, error: String) {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.iter_mut().find(|t| t.id == id) {
            task.status = TaskStatus::Failed;
            task.result = Some(error);
        }
    }

    /// Cancel a pending task
    pub async fn cancel(&self, id: usize) -> bool {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.iter_mut().find(|t| t.id == id && t.status == TaskStatus::Pending) {
            task.status = TaskStatus::Cancelled;
            true
        } else {
            false
        }
    }

    /// List all tasks
    pub async fn list(&self) -> Vec<Task> {
        self.tasks.lock().await.iter().cloned().collect()
    }

    /// Count pending tasks
    pub async fn pending_count(&self) -> usize {
        self.tasks.lock().await.iter().filter(|t| t.status == TaskStatus::Pending).count()
    }

    /// Format task list for display
    pub async fn format_list(&self) -> String {
        let tasks = self.tasks.lock().await;
        if tasks.is_empty() {
            return "No tasks in queue.".to_string();
        }
        let mut out = String::new();
        for t in tasks.iter() {
            out.push_str(&format!(
                "{} #{} [{}] ({:?}) {}\n",
                t.status.emoji(), t.id, t.task_type.as_str(), t.priority, t.description
            ));
            if let Some(ref result) = t.result {
                let preview = if result.len() > 100 { &result[..100] } else { result };
                out.push_str(&format!("   → {preview}\n"));
            }
        }
        out
    }
}
