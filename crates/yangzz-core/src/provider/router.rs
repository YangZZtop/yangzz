//! Smart Model Router: automatically select the best model for a given task.
//! Routes based on task complexity, cost, and model capabilities.

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
