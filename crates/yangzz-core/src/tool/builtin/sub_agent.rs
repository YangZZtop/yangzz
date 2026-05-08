use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{Value, json};

pub struct SubAgentTool;

const SUB_AGENT_MAX_TURNS: usize = 10;

#[async_trait]
impl Tool for SubAgentTool {
    fn name(&self) -> &str {
        "sub_agent"
    }

    fn description(&self) -> &str {
        "Spawn a sub-agent to handle a focused sub-task. The sub-agent runs its own agentic loop with tools and returns the result. Use for tasks that benefit from decomposition."
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

        let provider = ctx.provider.as_ref().ok_or_else(|| {
            ToolError::Execution("Sub-agent requires a provider (not available in this context)".into())
        })?;
        let model = ctx.model.as_deref().ok_or_else(|| {
            ToolError::Execution("Sub-agent requires a model (not available in this context)".into())
        })?;

        let system_prompt = format!(
            "You are a focused sub-agent. Complete ONLY the following task and report the result concisely.\n\
             Working directory: {}\n\
             Do not ask for confirmation. Execute the task directly using available tools.\n\
             When done, provide a brief summary of what you accomplished.",
            ctx.cwd.display()
        );

        let user_msg = if extra_context.is_empty() {
            task.to_string()
        } else {
            format!("{task}\n\nContext: {extra_context}")
        };

        let mut messages = vec![crate::message::Message::user(&user_msg)];
        let mut renderer = crate::render::NullRenderer::new();

        // Build a sub-executor with the same cwd but no nested sub-agent capability
        let sub_registry = crate::tool::ToolRegistry::with_builtins(&ctx.cwd);
        let sub_permission = std::sync::Arc::new(crate::permission::PermissionManager::new());
        let sub_executor = crate::tool::ToolExecutor::new(sub_registry, sub_permission, ctx.cwd.clone());

        let result = crate::query::run_agentic_loop_bounded(
            provider,
            model,
            ctx.max_tokens,
            &mut messages,
            Some(system_prompt),
            &sub_executor,
            &mut renderer,
            SUB_AGENT_MAX_TURNS,
        )
        .await;

        match result {
            Ok(_usage) => {
                let output = if renderer.collected_text.is_empty() {
                    // Extract from messages if renderer didn't collect
                    messages
                        .iter()
                        .rev()
                        .find_map(|m| {
                            m.content.iter().find_map(|b| b.as_text().map(String::from))
                        })
                        .unwrap_or_else(|| "[sub-agent completed with no text output]".to_string())
                } else {
                    renderer.collected_text
                };
                Ok(ToolOutput::success(output))
            }
            Err(e) => Ok(ToolOutput::error(format!("Sub-agent failed: {e}"))),
        }
    }
}
