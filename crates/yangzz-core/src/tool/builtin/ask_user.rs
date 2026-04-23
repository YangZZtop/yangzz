use crate::tool::{Tool, ToolContext, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::io::{self, Write};

pub struct AskUserTool;

#[async_trait]
impl Tool for AskUserTool {
    fn name(&self) -> &str { "ask_user" }

    fn description(&self) -> &str {
        "Ask the user a question and wait for their response. Use this when you need clarification or user input to proceed."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to ask the user"
                }
            },
            "required": ["question"]
        })
    }

    fn is_read_only(&self) -> bool { true }

    async fn execute(&self, input: &Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let question = input["question"].as_str()
            .ok_or_else(|| ToolError::Validation("Missing 'question'".into()))?;

        eprint!("\n  \x1b[33m?\x1b[0m \x1b[1m{question}\x1b[0m\n  \x1b[2m> \x1b[0m");
        let _ = io::stderr().flush();

        let mut response = String::new();
        io::stdin().read_line(&mut response)
            .map_err(|e| ToolError::Execution(format!("Cannot read input: {e}")))?;

        let answer = response.trim().to_string();
        if answer.is_empty() {
            Ok(ToolOutput::success("(user provided no answer)"))
        } else {
            Ok(ToolOutput::success(answer))
        }
    }
}
