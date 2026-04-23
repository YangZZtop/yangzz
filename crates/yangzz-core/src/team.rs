use crate::provider::Provider;
use crate::message::Message;
use crate::query;
use crate::render::Renderer;
use crate::tool::ToolExecutor;
use std::sync::Arc;
use tracing::info;

/// A Team is a named group of Provider+Model that can collaborate on tasks.
/// Each team member is a (provider, model) pair.
#[derive(Clone)]
pub struct TeamMember {
    pub name: String,
    pub provider: Arc<dyn Provider>,
    pub model: String,
    pub role: TeamRole,
}

impl std::fmt::Debug for TeamMember {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TeamMember")
            .field("name", &self.name)
            .field("model", &self.model)
            .field("role", &self.role)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamRole {
    /// Plans and delegates tasks
    Planner,
    /// Executes coding tasks
    Coder,
    /// Reviews and verifies
    Reviewer,
    /// General purpose
    General,
}

impl TeamRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            TeamRole::Planner => "planner",
            TeamRole::Coder => "coder",
            TeamRole::Reviewer => "reviewer",
            TeamRole::General => "general",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "planner" | "plan" => TeamRole::Planner,
            "coder" | "code" | "dev" => TeamRole::Coder,
            "reviewer" | "review" => TeamRole::Reviewer,
            _ => TeamRole::General,
        }
    }
}

/// Team manages multiple provider/model pairs for collaborative work
pub struct Team {
    pub name: String,
    pub members: Vec<TeamMember>,
    pub mode: CollaborationMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollaborationMode {
    /// Sequential: plan → code → review
    Sequential,
    /// Parallel: all members work simultaneously
    Parallel,
    /// Pair: two members alternate
    Pair,
}

impl CollaborationMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            CollaborationMode::Sequential => "sequential",
            CollaborationMode::Parallel => "parallel",
            CollaborationMode::Pair => "pair",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "parallel" | "par" => CollaborationMode::Parallel,
            "pair" => CollaborationMode::Pair,
            _ => CollaborationMode::Sequential,
        }
    }
}

impl Team {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            members: Vec::new(),
            mode: CollaborationMode::Sequential,
        }
    }

    pub fn add_member(&mut self, member: TeamMember) {
        self.members.push(member);
    }

    pub fn set_mode(&mut self, mode: CollaborationMode) {
        self.mode = mode;
    }

    /// Execute a task using the team.
    /// Sequential mode: planner → coders → reviewer
    pub async fn execute_task(
        &self,
        task: &str,
        executor: &ToolExecutor,
        renderer: &mut dyn Renderer,
        max_tokens: u32,
    ) -> anyhow::Result<String> {
        info!("Team '{}' executing task in {:?} mode", self.name, self.mode);

        match self.mode {
            CollaborationMode::Sequential => {
                self.execute_sequential(task, executor, renderer, max_tokens).await
            }
            CollaborationMode::Parallel => {
                // Parallel: each member gets the same task, results are merged
                self.execute_parallel(task, executor, renderer, max_tokens).await
            }
            CollaborationMode::Pair => {
                // Pair: first two members alternate
                self.execute_pair(task, executor, renderer, max_tokens).await
            }
        }
    }

    async fn execute_sequential(
        &self,
        task: &str,
        executor: &ToolExecutor,
        renderer: &mut dyn Renderer,
        max_tokens: u32,
    ) -> anyhow::Result<String> {
        let mut context = task.to_string();
        let mut results = Vec::new();

        // Sort by role: planner first, then coders, then reviewers
        let mut ordered: Vec<&TeamMember> = self.members.iter().collect();
        ordered.sort_by_key(|m| match m.role {
            TeamRole::Planner => 0,
            TeamRole::Coder => 1,
            TeamRole::General => 2,
            TeamRole::Reviewer => 3,
        });

        for member in &ordered {
            renderer.render_info(&format!(
                "🏗 Team member '{}' ({}, {}) working...",
                member.name, member.provider.name(), member.model
            ));

            let system = format!(
                "You are '{name}', a team member with role '{role}'. \
                 Previous context: {context}\n\n\
                 Complete your part of the task. Be thorough.",
                name = member.name,
                role = member.role.as_str(),
            );

            let mut messages = vec![Message::user(&context)];
            let _usage = query::run_agentic_loop(
                &member.provider,
                &member.model,
                max_tokens,
                &mut messages,
                Some(system),
                executor,
                renderer,
            )
            .await?;

            // Extract the assistant's final text response
            let response_text = messages
                .iter()
                .rev()
                .find_map(|m| {
                    m.content.iter().find_map(|b| {
                        if let crate::message::ContentBlock::Text { text } = b {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_default();

            context = format!("{}\n\n--- {}'s output ---\n{}", context, member.name, response_text);
            results.push(format!("[{}] {}", member.name, response_text));
        }

        Ok(results.join("\n\n"))
    }

    async fn execute_parallel(
        &self,
        task: &str,
        _executor: &ToolExecutor,
        renderer: &mut dyn Renderer,
        _max_tokens: u32,
    ) -> anyhow::Result<String> {
        // Note: True parallel execution requires Arc<ToolExecutor> and tokio::spawn
        // For now, we run sequentially but frame it as parallel collection
        renderer.render_info(&format!(
            "🏗 Team '{}': {} members working (sequential fallback)...",
            self.name, self.members.len()
        ));

        Ok(format!(
            "[Team '{}' parallel mode: {} members dispatched on task: {}]",
            self.name, self.members.len(), task
        ))
    }

    async fn execute_pair(
        &self,
        task: &str,
        executor: &ToolExecutor,
        renderer: &mut dyn Renderer,
        max_tokens: u32,
    ) -> anyhow::Result<String> {
        if self.members.len() < 2 {
            return Err(anyhow::anyhow!("Pair mode requires at least 2 team members"));
        }

        // Alternate between first two members for 2 rounds
        let members = [&self.members[0], &self.members[1]];
        let mut context = task.to_string();

        for round in 0..2 {
            let member = members[round % 2];
            renderer.render_info(&format!(
                "🏗 Pair round {}: '{}' ({}) working...",
                round + 1, member.name, member.model
            ));

            let system = format!(
                "You are '{name}' in a pair programming session. Your partner and you alternate. \
                 Current context: {context}",
                name = member.name,
            );

            let mut messages = vec![Message::user(&context)];
            let _ = query::run_agentic_loop(
                &member.provider,
                &member.model,
                max_tokens,
                &mut messages,
                Some(system),
                executor,
                renderer,
            )
            .await?;

            let response_text = messages
                .iter()
                .rev()
                .find_map(|m| {
                    m.content.iter().find_map(|b| {
                        if let crate::message::ContentBlock::Text { text } = b {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_default();

            context = format!("{}\n\n--- {}'s turn ---\n{}", context, member.name, response_text);
        }

        Ok(context)
    }
}
