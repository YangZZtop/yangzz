use crate::config::settings::StrategyConfig;
use crate::provider::router::{StrategyRouter, TaskDomain};
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
        executor: &ToolExecutor,
        renderer: &mut dyn Renderer,
        max_tokens: u32,
    ) -> anyhow::Result<String> {
        renderer.render_info(&format!(
            "🏗 Team '{}': {} members working...",
            self.name, self.members.len()
        ));

        // Execute each member's agentic loop in sequence (tool execution needs
        // serialized filesystem access). Each member sees the accumulated context
        // from previous members, making it a pipeline.
        let context = task.to_string();
        let mut results = Vec::new();

        for member in &self.members {
            renderer.render_info(&format!(
                "  ● {} ({}, {}) ...",
                member.name, member.role.as_str(), member.model
            ));

            let system = format!(
                "You are '{name}', a team member with role '{role}'. \
                 You are working as part of a multi-model team. \
                 Previous team members' work:\n{prev}\n\n\
                 Complete YOUR part of the task. Focus on your role. Be thorough.",
                name = member.name,
                role = member.role.as_str(),
                prev = if results.is_empty() { "(you are first)".to_string() }
                       else { results.join("\n---\n") },
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

            renderer.render_info(&format!("  ✓ {} completed", member.name));
            results.push(format!("[{} ({})] {}", member.name, member.role.as_str(), response_text));
        }

        Ok(results.join("\n\n---\n\n"))
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

// ── Null Renderer (for parallel workers that collect output silently) ──

#[allow(dead_code)]
struct NullRenderer;

impl Renderer for NullRenderer {
    fn render_text_delta(&mut self, _text: &str) {}
    fn render_tool_start(&mut self, _name: &str, _id: &str) {}
    fn render_tool_result(&mut self, _name: &str, _result: &str, _is_error: bool) {}
    fn render_error(&mut self, _message: &str) {}
    fn render_info(&mut self, _message: &str) {}
    fn render_complete(&mut self) {}
    fn render_status(&mut self, _status: &str) {}
}

// ── Strategy-driven Team Builder ──

/// Build a Team from strategy config + available providers
pub fn build_team_from_strategy(
    strategy: &StrategyConfig,
    providers: &[(String, Arc<dyn Provider>, String)], // (name, provider, default_model)
) -> Team {
    let router = StrategyRouter::new(strategy);
    let mut team = Team::new("strategy-team");

    // Set mode based on strategy
    team.set_mode(match strategy.mode.as_str() {
        "team" | "parallel" => CollaborationMode::Parallel,
        "pair" => CollaborationMode::Pair,
        _ => CollaborationMode::Sequential,
    });

    // For each configured role, find the provider and add as team member
    let role_mappings: &[(TaskDomain, TeamRole)] = &[
        (TaskDomain::Planner, TeamRole::Planner),
        (TaskDomain::Frontend, TeamRole::Coder),
        (TaskDomain::Backend, TeamRole::Coder),
        (TaskDomain::Review, TeamRole::Reviewer),
        (TaskDomain::Test, TeamRole::Coder),
    ];

    let mut added = std::collections::HashSet::new();

    for (domain, role) in role_mappings {
        if let Some(provider_name) = router.provider_for_domain(*domain) {
            // Don't add duplicate (same provider+role)
            let key = format!("{}:{}", provider_name, role.as_str());
            if added.contains(&key) {
                continue;
            }
            added.insert(key);

            if let Some((name, provider, model)) = providers.iter().find(|(n, _, _)| n == &provider_name) {
                team.add_member(TeamMember {
                    name: format!("{}-{}", name, domain.as_str()),
                    provider: provider.clone(),
                    model: model.clone(),
                    role: *role,
                });
            }
        }
    }

    // Fallback: if team is empty, add all providers as general members
    if team.members.is_empty() {
        for (name, provider, model) in providers {
            team.add_member(TeamMember {
                name: name.clone(),
                provider: provider.clone(),
                model: model.clone(),
                role: TeamRole::General,
            });
        }
    }

    info!("Built strategy team with {} members: {:?}",
        team.members.len(),
        team.members.iter().map(|m| format!("{}({})", m.name, m.model)).collect::<Vec<_>>()
    );

    team
}

/// Execute a task using strategy routing:
/// - Single domain → direct route to the assigned provider
/// - Multi-domain → 3-phase protocol: Plan → Execute → Verify
pub async fn execute_with_strategy(
    input: &str,
    strategy: &StrategyConfig,
    providers: &[(String, Arc<dyn Provider>, String)],
    executor: &ToolExecutor,
    renderer: &mut dyn Renderer,
    max_tokens: u32,
) -> anyhow::Result<String> {
    let router = StrategyRouter::new(strategy);
    let tasks = router.decompose_task(input);

    if tasks.len() == 1 {
        // Single domain — just route to the right provider
        let (domain, task_text) = &tasks[0];
        let provider_name = router.provider_for_domain(*domain)
            .unwrap_or_default();

        renderer.render_info(&format!(
            "📍 Strategy: {} → {}",
            domain.as_str(),
            if provider_name.is_empty() { "(default)" } else { &provider_name }
        ));

        if let Some((_, provider, model)) = providers.iter().find(|(n, _, _)| n == &provider_name) {
            let mut messages = vec![Message::user(task_text)];
            let _usage = query::run_agentic_loop(
                provider,
                model,
                max_tokens,
                &mut messages,
                None,
                executor,
                renderer,
            ).await?;

            let response = messages.iter().rev().find_map(|m| {
                m.content.iter().find_map(|b| {
                    if let crate::message::ContentBlock::Text { text } = b {
                        Some(text.clone())
                    } else { None }
                })
            }).unwrap_or_default();

            Ok(response)
        } else {
            Err(anyhow::anyhow!("Provider '{}' not found for domain '{}'", provider_name, domain.as_str()))
        }
    } else {
        // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
        // Multi-domain: 3-phase coordination protocol
        // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
        renderer.render_info(&format!(
            "🏗 Multi-agent task: {} domains detected",
            tasks.len()
        ));
        for (domain, _) in &tasks {
            let pn = router.provider_for_domain(*domain).unwrap_or_default();
            renderer.render_info(&format!("  • {} → {}", domain.as_str(), pn));
        }

        // Pick the planner provider (or first available)
        let planner = providers.first()
            .ok_or_else(|| anyhow::anyhow!("No providers available"))?;

        // ── Phase 1: PLAN — generate shared contract ──
        renderer.render_info("━━ Phase 1/3: Generating shared contract...");

        let plan_prompt = format!(
            "You are the ARCHITECT for a multi-agent team task.\n\
             The user's request: {input}\n\n\
             The following agents will work on this task:\n{agents}\n\n\
             Generate a SHARED CONTRACT that all agents must follow. Include:\n\
             1. **API Contracts**: endpoint paths, request/response schemas, status codes\n\
             2. **Data Models**: shared types, field names, validation rules\n\
             3. **File Structure**: which files each agent creates/modifies\n\
             4. **Naming Conventions**: consistent naming across frontend/backend\n\
             5. **Integration Points**: how frontend calls backend, test fixtures match API\n\n\
             Output the contract in a clear, structured format. \
             This contract will be given to every agent as their specification.\n\
             Be specific — use exact names, types, and paths. No ambiguity.",
            agents = tasks.iter()
                .map(|(d, _)| format!("  - {}: {}", d.as_str(),
                    router.provider_for_domain(*d).unwrap_or_default()))
                .collect::<Vec<_>>().join("\n")
        );

        let mut plan_messages = vec![Message::user(&plan_prompt)];
        let _plan_usage = query::run_agentic_loop(
            &planner.1, &planner.2, max_tokens,
            &mut plan_messages, None, executor, renderer,
        ).await?;

        let contract = extract_last_text(&plan_messages);
        renderer.render_info(&format!("  ✓ Contract generated ({} chars)", contract.len()));

        // ── Phase 2: EXECUTE — each agent works with the contract ──
        renderer.render_info("━━ Phase 2/3: Executing with shared contract...");

        let mut all_results = Vec::new();

        for (domain, task_text) in &tasks {
            let provider_name = router.provider_for_domain(*domain)
                .unwrap_or_default();

            let (_, provider, model) = match providers.iter()
                .find(|(n, _, _)| n == &provider_name)
            {
                Some(p) => p,
                None => {
                    renderer.render_info(&format!(
                        "  ⚠ Skipping {}: provider '{}' not found",
                        domain.as_str(), provider_name
                    ));
                    continue;
                }
            };

            renderer.render_info(&format!(
                "  ● {} ({}) working...",
                domain.as_str(), model
            ));

            let exec_prompt = format!(
                "You are working as the **{}** agent in a multi-agent team.\n\n\
                 ## SHARED CONTRACT (ALL agents follow this)\n{contract}\n\n\
                 ## YOUR TASK\n{task_text}\n\n\
                 IMPORTANT:\n\
                 - Follow the shared contract EXACTLY — use the same API paths, field names, types\n\
                 - Do NOT invent new endpoints or rename fields\n\
                 - If the contract says `GET /api/users` returns `{{ id, name, email }}`, \
                   that's exactly what you implement/consume\n\
                 - Complete your part thoroughly using tools",
                domain.as_str(),
            );

            let mut exec_messages = vec![Message::user(&exec_prompt)];
            let _exec_usage = query::run_agentic_loop(
                provider, model, max_tokens,
                &mut exec_messages, None, executor, renderer,
            ).await?;

            let result = extract_last_text(&exec_messages);
            renderer.render_info(&format!("  ✓ {} completed", domain.as_str()));
            all_results.push(format!("[{} agent]\n{}", domain.as_str(), result));
        }

        // ── Phase 3: VERIFY — check alignment ──
        renderer.render_info("━━ Phase 3/3: Verifying alignment...");

        let verify_prompt = format!(
            "You are the REVIEWER verifying that a multi-agent team's work is aligned.\n\n\
             ## SHARED CONTRACT\n{contract}\n\n\
             ## AGENT OUTPUTS\n{outputs}\n\n\
             Check for:\n\
             1. **API mismatch**: frontend calling endpoints that backend didn't create\n\
             2. **Type mismatch**: frontend expecting fields that backend doesn't return\n\
             3. **Naming mismatch**: different names for the same concept\n\
             4. **Missing integration**: pieces that don't connect\n\n\
             If everything aligns, say ✅ ALIGNED and summarize.\n\
             If there are mismatches, list each one with:\n\
             - What's wrong\n\
             - Which agent needs to fix it\n\
             - The exact fix needed\n\
             Then apply the fixes using tools.",
            outputs = all_results.join("\n\n---\n\n")
        );

        let mut verify_messages = vec![Message::user(&verify_prompt)];
        let _verify_usage = query::run_agentic_loop(
            &planner.1, &planner.2, max_tokens,
            &mut verify_messages, None, executor, renderer,
        ).await?;

        let verification = extract_last_text(&verify_messages);
        renderer.render_info("  ✓ Verification complete");

        all_results.push(format!("[verification]\n{}", verification));
        Ok(all_results.join("\n\n━━━━━━━━━━━━\n\n"))
    }
}

/// Extract the last text content from messages
fn extract_last_text(messages: &[Message]) -> String {
    messages.iter().rev().find_map(|m| {
        m.content.iter().find_map(|b| {
            if let crate::message::ContentBlock::Text { text } = b {
                Some(text.clone())
            } else { None }
        })
    }).unwrap_or_default()
}
