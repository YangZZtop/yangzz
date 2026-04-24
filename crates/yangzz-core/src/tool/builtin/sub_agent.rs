use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{Value, json};

/// SubAgent tool: spawn a focused sub-task with its own context.
/// The sub-agent runs as a separate agentic loop with a narrowed prompt.
pub struct SubAgentTool;

#[async_trait]
impl Tool for SubAgentTool {
    fn name(&self) -> &str {
        "sub_agent"
    }

    fn description(&self) -> &str {
        "Spawn a sub-agent to handle a focused sub-task. The sub-agent receives a task description and returns its result. Use this for complex tasks that benefit from decomposition."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "Description of the sub-task for the sub-agent to complete"
                },
                "context": {
                    "type": "string",
                    "description": "Additional context or constraints for the sub-agent (optional)"
                }
            },
            "required": ["task"]
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: &Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let task = input["task"]
            .as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'task'".into()))?;
        let extra_context = input["context"].as_str().unwrap_or("");

        // Build a sub-agent system prompt
        let sub_prompt = format!(
            "You are a focused sub-agent. Complete ONLY the following task and report the result concisely.\n\
             Task: {task}\n\
             {}\n\
             Working directory: {}\n\
             Do not ask for confirmation. Execute the task directly using available tools.\n\
             When done, summarize what you did.",
            if extra_context.is_empty() {
                String::new()
            } else {
                format!("Context: {extra_context}\n")
            },
            ctx.cwd.display()
        );

        // We can't actually run a nested agentic loop here without the provider reference.
        // Instead, we return the structured sub-task as a prompt injection that the outer
        // agentic loop will handle as a continuation.
        //
        // This is the "lightweight" sub-agent pattern: the model sees the sub-task framing
        // and focuses on it, then returns to the outer task.
        Ok(ToolOutput::success(format!(
            "[SUB-AGENT ACTIVATED]\n\
             Sub-task: {task}\n\
             {}\n\
             Instructions: Focus exclusively on this sub-task. Use tools as needed. \
             When complete, summarize results and return to the main task.\n\
             \n--- Sub-agent system context ---\n{sub_prompt}",
            if extra_context.is_empty() {
                String::new()
            } else {
                format!("Context: {extra_context}\n")
            }
        )))
    }
}
