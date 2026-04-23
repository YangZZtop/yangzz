//! Smart Model Router: automatically select the best model for a given task.
//! Routes based on task complexity, cost, model capabilities, and user-defined strategy.

use crate::config::settings::{StrategyConfig, StrategyRoles, StrategyKeywords};
use tracing::info;

/// Task complexity classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskComplexity {
    /// Simple tasks: greetings, short questions, formatting
    Simple,
    /// Medium tasks: code edits, explanations, debugging
    Medium,
    /// Complex tasks: architecture, multi-file refactoring, planning
    Complex,
}

/// Route decision
#[derive(Debug, Clone)]
pub struct RouteDecision {
    pub model: String,
    pub reason: String,
    pub complexity: TaskComplexity,
}

/// Model tier for routing
#[derive(Debug, Clone)]
pub struct ModelTier {
    pub name: String,
    pub provider: String,
    pub complexity: TaskComplexity,
    pub cost_per_1k_input: f64,
}

/// Smart router configuration
pub struct ModelRouter {
    tiers: Vec<ModelTier>,
}

impl ModelRouter {
    /// Create a new router with default tiers
    pub fn new() -> Self {
        Self {
            tiers: vec![
                ModelTier {
                    name: "gpt-4o-mini".to_string(),
                    provider: "openai".to_string(),
                    complexity: TaskComplexity::Simple,
                    cost_per_1k_input: 0.00015,
                },
                ModelTier {
                    name: "gpt-4o".to_string(),
                    provider: "openai".to_string(),
                    complexity: TaskComplexity::Medium,
                    cost_per_1k_input: 0.005,
                },
                ModelTier {
                    name: "claude-sonnet-4-20250514".to_string(),
                    provider: "anthropic".to_string(),
                    complexity: TaskComplexity::Medium,
                    cost_per_1k_input: 0.003,
                },
                ModelTier {
                    name: "claude-opus-4-20250514".to_string(),
                    provider: "anthropic".to_string(),
                    complexity: TaskComplexity::Complex,
                    cost_per_1k_input: 0.015,
                },
                ModelTier {
                    name: "o3".to_string(),
                    provider: "openai".to_string(),
                    complexity: TaskComplexity::Complex,
                    cost_per_1k_input: 0.010,
                },
                ModelTier {
                    name: "deepseek-chat".to_string(),
                    provider: "deepseek".to_string(),
                    complexity: TaskComplexity::Simple,
                    cost_per_1k_input: 0.00014,
                },
            ],
        }
    }

    /// Add a custom tier
    pub fn add_tier(&mut self, tier: ModelTier) {
        self.tiers.push(tier);
    }

    /// Classify task complexity from user input
    pub fn classify_task(&self, input: &str) -> TaskComplexity {
        let lower = input.to_lowercase();
        let word_count = input.split_whitespace().count();

        // Complex indicators
        let complex_signals = [
            "refactor", "architecture", "redesign", "migrate",
            "implement all", "build a", "create a system",
            "multi-file", "across the codebase", "全部", "整体",
            "重构", "架构", "系统设计", "所有文件",
        ];
        for signal in &complex_signals {
            if lower.contains(signal) {
                return TaskComplexity::Complex;
            }
        }

        // Simple indicators
        let simple_signals = [
            "hello", "hi", "help", "what is", "explain",
            "format", "lint", "fix typo", "rename",
            "你好", "解释", "什么是", "帮我看",
        ];
        for signal in &simple_signals {
            if lower.contains(signal) {
                return TaskComplexity::Simple;
            }
        }

        // Based on length
        if word_count < 10 {
            TaskComplexity::Simple
        } else if word_count < 50 {
            TaskComplexity::Medium
        } else {
            TaskComplexity::Complex
        }
    }

    /// Route a task to the best model
    pub fn route(&self, input: &str, available_providers: &[&str]) -> RouteDecision {
        let complexity = self.classify_task(input);

        // Find best matching tier that's available
        let best = self.tiers.iter()
            .filter(|t| t.complexity == complexity)
            .filter(|t| available_providers.contains(&t.provider.as_str()))
            .min_by(|a, b| a.cost_per_1k_input.partial_cmp(&b.cost_per_1k_input).unwrap());

        // Fallback: any available model
        let decision = match best {
            Some(tier) => RouteDecision {
                model: tier.name.clone(),
                reason: format!(
                    "Task classified as {:?}, routing to {} (${:.4}/1K)",
                    complexity, tier.name, tier.cost_per_1k_input
                ),
                complexity,
            },
            None => {
                // Use first available tier
                let fallback = self.tiers.iter()
                    .find(|t| available_providers.contains(&t.provider.as_str()))
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| "gpt-4o".to_string());
                RouteDecision {
                    model: fallback.clone(),
                    reason: format!("No matching tier for {:?}, using fallback: {}", complexity, fallback),
                    complexity,
                }
            }
        };

        info!("Router decision: {}", decision.reason);
        decision
    }
}

// ── Strategy-based Router (user-defined roles) ──

/// Task domain — what kind of work this is
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskDomain {
    Planner,
    Frontend,
    Backend,
    Review,
    Test,
    General,
}

impl TaskDomain {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskDomain::Planner => "planner",
            TaskDomain::Frontend => "frontend",
            TaskDomain::Backend => "backend",
            TaskDomain::Review => "review",
            TaskDomain::Test => "test",
            TaskDomain::General => "general",
        }
    }
}

/// Strategy route result
#[derive(Debug, Clone)]
pub struct StrategyDecision {
    pub provider_name: String,
    pub domain: TaskDomain,
    pub reason: String,
}

/// Strategy router — routes based on user-defined config
pub struct StrategyRouter {
    roles: StrategyRoles,
    keywords: StrategyKeywords,
}

impl StrategyRouter {
    pub fn new(config: &StrategyConfig) -> Self {
        Self {
            roles: config.roles.clone(),
            keywords: config.keywords.clone(),
        }
    }

    /// Classify what domain a task belongs to by matching keywords
    pub fn classify_domain(&self, input: &str) -> TaskDomain {
        let lower = input.to_lowercase();
        let mut scores: Vec<(TaskDomain, usize)> = vec![
            (TaskDomain::Frontend, 0),
            (TaskDomain::Backend, 0),
            (TaskDomain::Review, 0),
            (TaskDomain::Test, 0),
            (TaskDomain::Planner, 0),
        ];

        for (domain, score) in &mut scores {
            let keywords = match domain {
                TaskDomain::Frontend => &self.keywords.frontend,
                TaskDomain::Backend => &self.keywords.backend,
                TaskDomain::Review => &self.keywords.review,
                TaskDomain::Test => &self.keywords.test,
                TaskDomain::Planner => &self.keywords.planner,
                TaskDomain::General => continue,
            };
            for kw in keywords {
                if lower.contains(&kw.to_lowercase()) {
                    *score += 1;
                }
            }
        }

        // Pick the highest scoring domain
        scores.sort_by(|a, b| b.1.cmp(&a.1));
        if scores[0].1 > 0 {
            scores[0].0
        } else {
            TaskDomain::General
        }
    }

    /// Route a task to the configured provider for its domain
    pub fn route(&self, input: &str) -> StrategyDecision {
        let domain = self.classify_domain(input);

        let provider_name = match domain {
            TaskDomain::Planner => self.roles.planner.clone(),
            TaskDomain::Frontend => self.roles.frontend.clone(),
            TaskDomain::Backend => self.roles.backend.clone(),
            TaskDomain::Review => self.roles.review.clone(),
            TaskDomain::Test => self.roles.test.clone(),
            TaskDomain::General => self.roles.general.clone(),
        }
        .or_else(|| self.roles.general.clone())
        .unwrap_or_default();

        let reason = format!(
            "Domain: {} → provider: {}",
            domain.as_str(),
            if provider_name.is_empty() { "(default)" } else { &provider_name }
        );

        info!("Strategy route: {}", reason);

        StrategyDecision {
            provider_name,
            domain,
            reason,
        }
    }

    /// Decompose a complex task into sub-tasks with domain assignments
    pub fn decompose_task(&self, input: &str) -> Vec<(TaskDomain, String)> {
        let mut tasks = Vec::new();
        let lower = input.to_lowercase();

        // Check for multi-domain signals
        let has_frontend = self.keywords.frontend.iter().any(|k| lower.contains(&k.to_lowercase()));
        let has_backend = self.keywords.backend.iter().any(|k| lower.contains(&k.to_lowercase()));
        let has_test = self.keywords.test.iter().any(|k| lower.contains(&k.to_lowercase()));
        let has_review = self.keywords.review.iter().any(|k| lower.contains(&k.to_lowercase()));
        let has_planner = self.keywords.planner.iter().any(|k| lower.contains(&k.to_lowercase()));

        // If it touches multiple domains, decompose
        let domain_count = [has_frontend, has_backend, has_test, has_review, has_planner]
            .iter().filter(|&&x| x).count();

        if domain_count >= 2 {
            // Multi-domain task: decompose
            if has_planner {
                tasks.push((TaskDomain::Planner, format!("Plan the overall approach: {input}")));
            }
            if has_frontend {
                tasks.push((TaskDomain::Frontend, format!("Handle frontend work: {input}")));
            }
            if has_backend {
                tasks.push((TaskDomain::Backend, format!("Handle backend work: {input}")));
            }
            if has_test {
                tasks.push((TaskDomain::Test, format!("Write tests: {input}")));
            }
            if has_review {
                tasks.push((TaskDomain::Review, format!("Review the changes: {input}")));
            }
        } else {
            // Single domain
            let domain = self.classify_domain(input);
            tasks.push((domain, input.to_string()));
        }

        tasks
    }

    /// Get the provider name for a specific domain
    pub fn provider_for_domain(&self, domain: TaskDomain) -> Option<String> {
        match domain {
            TaskDomain::Planner => self.roles.planner.clone(),
            TaskDomain::Frontend => self.roles.frontend.clone(),
            TaskDomain::Backend => self.roles.backend.clone(),
            TaskDomain::Review => self.roles.review.clone(),
            TaskDomain::Test => self.roles.test.clone(),
            TaskDomain::General => self.roles.general.clone(),
        }
        .or_else(|| self.roles.general.clone())
    }
}

// ── Natural Language Directive Parser ──
//
// Parse instructions like:
//   "Claude负责前端，GPT写后端，DeepSeek写测试"
//   "用claude做架构，gpt写代码"
//   "claude for frontend, gpt for backend"

/// A single role assignment parsed from natural language
#[derive(Debug, Clone)]
pub struct RoleDirective {
    pub model_hint: String,     // "claude", "gpt", "gemini", "deepseek", etc.
    pub domain: TaskDomain,
}

/// Result of parsing user input for team directives
#[derive(Debug, Clone)]
pub struct DirectiveParseResult {
    /// Parsed role assignments
    pub directives: Vec<RoleDirective>,
    /// The actual task (with directive parts stripped)
    pub task: String,
    /// Whether any directives were found
    pub has_directives: bool,
}

/// Parse natural language input for team role directives.
/// Returns extracted directives + the remaining task text.
pub fn parse_directives(input: &str) -> DirectiveParseResult {
    let lower = input.to_lowercase();
    let mut directives = Vec::new();

    // Model name patterns (fuzzy match)
    let model_patterns: &[(&[&str], &str)] = &[
        (&["claude", "sonnet", "opus", "haiku"], "claude"),
        (&["gpt", "openai", "o3", "o4"], "openai"),
        (&["gemini", "google"], "gemini"),
        (&["deepseek", "ds"], "deepseek"),
        (&["grok", "xai"], "grok"),
        (&["glm", "zhipu", "chatglm"], "glm"),
        (&["ollama", "llama", "本地"], "ollama"),
        (&["qwen", "通义"], "qwen"),
    ];

    // Domain patterns (Chinese + English)
    let domain_patterns: &[(&[&str], TaskDomain)] = &[
        (&["前端", "frontend", "ui", "页面", "组件", "css", "react", "vue"], TaskDomain::Frontend),
        (&["后端", "backend", "api", "接口", "服务", "server", "数据库", "database"], TaskDomain::Backend),
        (&["测试", "test", "spec", "单测"], TaskDomain::Test),
        (&["审查", "review", "检查", "code review", "cr"], TaskDomain::Review),
        (&["架构", "设计", "规划", "plan", "architect", "design"], TaskDomain::Planner),
    ];

    // Assignment verb patterns
    let assign_verbs = [
        "负责", "写", "做", "搞", "处理", "管",
        "for", "handle", "do", "work on", "take care of",
        "来", "去",
    ];

    // Strategy: find "model + verb + domain" or "用model + verb + domain" patterns
    // Split by common delimiters
    let segments: Vec<&str> = lower
        .split(|c: char| c == '，' || c == ',' || c == '、' || c == '；' || c == ';' || c == '。' || c == '.')
        .collect();

    let mut directive_spans: Vec<(usize, usize)> = Vec::new(); // byte ranges to strip

    for segment in &segments {
        let seg = segment.trim();
        if seg.is_empty() { continue; }

        // Try to find a model mention
        let mut found_model: Option<&str> = None;
        for (aliases, canonical) in model_patterns {
            for alias in *aliases {
                if seg.contains(alias) {
                    found_model = Some(canonical);
                    break;
                }
            }
            if found_model.is_some() { break; }
        }

        // Try to find a domain mention
        let mut found_domain: Option<TaskDomain> = None;
        for (keywords, domain) in domain_patterns {
            for kw in *keywords {
                if seg.contains(kw) {
                    found_domain = Some(*domain);
                    break;
                }
            }
            if found_domain.is_some() { break; }
        }

        // Check for assignment verb
        let has_verb = assign_verbs.iter().any(|v| seg.contains(v));

        // If we found both model and domain (with or without verb), it's a directive
        if let (Some(model), Some(domain)) = (found_model, found_domain) {
            if has_verb || true {
                // Even without explicit verb, "Claude前端" is clear enough
                directives.push(RoleDirective {
                    model_hint: model.to_string(),
                    domain,
                });

                // Find this segment in the original input to mark for stripping
                if let Some(start) = input.to_lowercase().find(seg) {
                    directive_spans.push((start, start + seg.len()));
                }
            }
        }
    }

    // Build the remaining task text by stripping directive segments
    let task = if directive_spans.is_empty() {
        input.to_string()
    } else {
        let mut result = input.to_string();
        // Sort spans in reverse order to avoid index shifting
        let mut spans = directive_spans;
        spans.sort_by(|a, b| b.0.cmp(&a.0));
        for (start, end) in spans {
            if start < result.len() && end <= result.len() {
                result.replace_range(start..end, "");
            }
        }
        // Clean up leftover delimiters
        let result = result
            .replace("，，", "，")
            .replace(",,", ",")
            .trim_matches(|c: char| c == '，' || c == ',' || c == ' ' || c == '、')
            .to_string();
        if result.is_empty() { input.to_string() } else { result }
    };

    let has_directives = !directives.is_empty();
    DirectiveParseResult { directives, task, has_directives }
}

/// Convert parsed directives into a StrategyConfig (for one-time use)
pub fn directives_to_strategy(directives: &[RoleDirective]) -> StrategyConfig {
    let mut roles = StrategyRoles::default();

    for d in directives {
        let name = d.model_hint.clone();
        match d.domain {
            TaskDomain::Planner => roles.planner = Some(name),
            TaskDomain::Frontend => roles.frontend = Some(name),
            TaskDomain::Backend => roles.backend = Some(name),
            TaskDomain::Review => roles.review = Some(name),
            TaskDomain::Test => roles.test = Some(name),
            TaskDomain::General => roles.general = Some(name),
        }
    }

    // Set general to the first directive's model if not explicitly set
    if roles.general.is_none() {
        if let Some(first) = directives.first() {
            roles.general = Some(first.model_hint.clone());
        }
    }

    StrategyConfig {
        mode: "auto".to_string(),
        roles,
        keywords: StrategyKeywords::default(),
    }
}
