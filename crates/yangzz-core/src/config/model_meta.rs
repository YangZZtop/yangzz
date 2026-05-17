/// Model metadata — pricing, context window, capabilities
///
/// Prices are per 1M tokens in USD.
/// Context is max input tokens.

#[derive(Debug, Clone)]
pub struct ModelMeta {
    pub model_pattern: &'static str,
    pub display_name: &'static str,
    pub context_window: u64,            // max input tokens
    pub input_price: f64,               // $/1M input tokens
    pub output_price: f64,              // $/1M output tokens
    pub cache_write_price: Option<f64>, // $/1M cached input write
    pub cache_read_price: Option<f64>,  // $/1M cached input read
    pub supports_reasoning: bool,
    pub reasoning_effort: Option<&'static str>, // "low" / "medium" / "high"
}

/// Known model metadata (sorted by provider, then model)
pub static MODEL_CATALOG: &[ModelMeta] = &[
    // ── OpenAI ──
    ModelMeta {
        model_pattern: "gpt-4o",
        display_name: "GPT-4o",
        context_window: 128_000,
        input_price: 2.50,
        output_price: 10.00,
        cache_write_price: None,
        cache_read_price: Some(1.25),
        supports_reasoning: false,
        reasoning_effort: None,
    },
    ModelMeta {
        model_pattern: "gpt-4o-mini",
        display_name: "GPT-4o Mini",
        context_window: 128_000,
        input_price: 0.15,
        output_price: 0.60,
        cache_write_price: None,
        cache_read_price: Some(0.075),
        supports_reasoning: false,
        reasoning_effort: None,
    },
    ModelMeta {
        model_pattern: "gpt-5.4",
        display_name: "GPT-5.4",
        context_window: 1_000_000,
        input_price: 2.00,
        output_price: 8.00,
        cache_write_price: None,
        cache_read_price: Some(0.50),
        supports_reasoning: true,
        reasoning_effort: Some("medium"),
    },
    ModelMeta {
        model_pattern: "o4-mini",
        display_name: "o4-mini",
        context_window: 200_000,
        input_price: 1.10,
        output_price: 4.40,
        cache_write_price: None,
        cache_read_price: Some(0.275),
        supports_reasoning: true,
        reasoning_effort: Some("medium"),
    },
    ModelMeta {
        model_pattern: "o3",
        display_name: "o3",
        context_window: 200_000,
        input_price: 10.00,
        output_price: 40.00,
        cache_write_price: None,
        cache_read_price: Some(2.50),
        supports_reasoning: true,
        reasoning_effort: Some("high"),
    },
    ModelMeta {
        model_pattern: "o3-mini",
        display_name: "o3-mini",
        context_window: 200_000,
        input_price: 1.10,
        output_price: 4.40,
        cache_write_price: None,
        cache_read_price: Some(0.275),
        supports_reasoning: true,
        reasoning_effort: Some("medium"),
    },
    // ── Anthropic ──
    ModelMeta {
        model_pattern: "claude-sonnet-4-5",
        display_name: "Claude Sonnet 4.5",
        context_window: 1_000_000,
        input_price: 3.00,
        output_price: 15.00,
        cache_write_price: Some(3.75),
        cache_read_price: Some(0.30),
        supports_reasoning: true,
        reasoning_effort: Some("medium"),
    },
    ModelMeta {
        model_pattern: "claude-sonnet-4",
        display_name: "Claude Sonnet 4",
        context_window: 1_000_000,
        input_price: 3.00,
        output_price: 15.00,
        cache_write_price: Some(3.75),
        cache_read_price: Some(0.30),
        supports_reasoning: true,
        reasoning_effort: Some("medium"),
    },
    ModelMeta {
        model_pattern: "claude-opus-4-7",
        display_name: "Claude Opus 4.7",
        context_window: 1_000_000,
        input_price: 15.00,
        output_price: 75.00,
        cache_write_price: Some(18.75),
        cache_read_price: Some(1.50),
        supports_reasoning: true,
        reasoning_effort: Some("high"),
    },
    ModelMeta {
        model_pattern: "claude-opus-4-6",
        display_name: "Claude Opus 4.6",
        context_window: 1_000_000,
        input_price: 15.00,
        output_price: 75.00,
        cache_write_price: Some(18.75),
        cache_read_price: Some(1.50),
        supports_reasoning: true,
        reasoning_effort: Some("high"),
    },
    ModelMeta {
        model_pattern: "claude-opus-4",
        display_name: "Claude Opus 4",
        context_window: 1_000_000,
        input_price: 15.00,
        output_price: 75.00,
        cache_write_price: Some(18.75),
        cache_read_price: Some(1.50),
        supports_reasoning: true,
        reasoning_effort: Some("high"),
    },
    ModelMeta {
        model_pattern: "claude-3.5-sonnet",
        display_name: "Claude 3.5 Sonnet",
        context_window: 200_000,
        input_price: 3.00,
        output_price: 15.00,
        cache_write_price: Some(3.75),
        cache_read_price: Some(0.30),
        supports_reasoning: false,
        reasoning_effort: None,
    },
    ModelMeta {
        model_pattern: "claude-3.5-haiku",
        display_name: "Claude 3.5 Haiku",
        context_window: 200_000,
        input_price: 0.80,
        output_price: 4.00,
        cache_write_price: Some(1.00),
        cache_read_price: Some(0.08),
        supports_reasoning: false,
        reasoning_effort: None,
    },
    // ── Google Gemini ──
    ModelMeta {
        model_pattern: "gemini-2.5-pro",
        display_name: "Gemini 2.5 Pro",
        context_window: 1_000_000,
        input_price: 1.25,
        output_price: 10.00,
        cache_write_price: None,
        cache_read_price: Some(0.315),
        supports_reasoning: true,
        reasoning_effort: Some("medium"),
    },
    ModelMeta {
        model_pattern: "gemini-2.5-flash",
        display_name: "Gemini 2.5 Flash",
        context_window: 1_000_000,
        input_price: 0.15,
        output_price: 0.60,
        cache_write_price: None,
        cache_read_price: Some(0.0375),
        supports_reasoning: true,
        reasoning_effort: Some("low"),
    },
    ModelMeta {
        model_pattern: "gemini-2.0-flash",
        display_name: "Gemini 2.0 Flash",
        context_window: 1_000_000,
        input_price: 0.10,
        output_price: 0.40,
        cache_write_price: None,
        cache_read_price: Some(0.025),
        supports_reasoning: false,
        reasoning_effort: None,
    },
    // ── DeepSeek ──
    ModelMeta {
        model_pattern: "deepseek-chat",
        display_name: "DeepSeek V3",
        context_window: 64_000,
        input_price: 0.27,
        output_price: 1.10,
        cache_write_price: None,
        cache_read_price: Some(0.07),
        supports_reasoning: false,
        reasoning_effort: None,
    },
    ModelMeta {
        model_pattern: "deepseek-reasoner",
        display_name: "DeepSeek R1",
        context_window: 64_000,
        input_price: 0.55,
        output_price: 2.19,
        cache_write_price: None,
        cache_read_price: Some(0.14),
        supports_reasoning: true,
        reasoning_effort: Some("high"),
    },
    // ── xAI Grok ──
    ModelMeta {
        model_pattern: "grok-3",
        display_name: "Grok 3",
        context_window: 131_072,
        input_price: 3.00,
        output_price: 15.00,
        cache_write_price: None,
        cache_read_price: None,
        supports_reasoning: true,
        reasoning_effort: Some("medium"),
    },
    ModelMeta {
        model_pattern: "grok-3-mini",
        display_name: "Grok 3 Mini",
        context_window: 131_072,
        input_price: 0.30,
        output_price: 0.50,
        cache_write_price: None,
        cache_read_price: None,
        supports_reasoning: true,
        reasoning_effort: Some("low"),
    },
    // ── Qwen (通义千问) ──
    ModelMeta {
        model_pattern: "qwen-max",
        display_name: "Qwen Max",
        context_window: 128_000,
        input_price: 2.40,
        output_price: 9.60,
        cache_write_price: None,
        cache_read_price: None,
        supports_reasoning: false,
        reasoning_effort: None,
    },
    ModelMeta {
        model_pattern: "qwen-plus",
        display_name: "Qwen Plus",
        context_window: 128_000,
        input_price: 0.80,
        output_price: 2.00,
        cache_write_price: None,
        cache_read_price: None,
        supports_reasoning: false,
        reasoning_effort: None,
    },
    ModelMeta {
        model_pattern: "qwen-turbo",
        display_name: "Qwen Turbo",
        context_window: 128_000,
        input_price: 0.30,
        output_price: 0.60,
        cache_write_price: None,
        cache_read_price: None,
        supports_reasoning: false,
        reasoning_effort: None,
    },
    ModelMeta {
        model_pattern: "qwq",
        display_name: "QwQ (Reasoning)",
        context_window: 128_000,
        input_price: 0.50,
        output_price: 2.00,
        cache_write_price: None,
        cache_read_price: None,
        supports_reasoning: true,
        reasoning_effort: Some("high"),
    },
    // ── GLM (智谱) ──
    ModelMeta {
        model_pattern: "glm-4-plus",
        display_name: "GLM-4 Plus",
        context_window: 128_000,
        input_price: 0.50,
        output_price: 0.50,
        cache_write_price: None,
        cache_read_price: None,
        supports_reasoning: false,
        reasoning_effort: None,
    },
    ModelMeta {
        model_pattern: "glm-4-flash",
        display_name: "GLM-4 Flash",
        context_window: 128_000,
        input_price: 0.01,
        output_price: 0.01,
        cache_write_price: None,
        cache_read_price: None,
        supports_reasoning: false,
        reasoning_effort: None,
    },
    // ── Meta Llama ──
    ModelMeta {
        model_pattern: "llama-3.3-70b",
        display_name: "Llama 3.3 70B",
        context_window: 128_000,
        input_price: 0.60,
        output_price: 0.60,
        cache_write_price: None,
        cache_read_price: None,
        supports_reasoning: false,
        reasoning_effort: None,
    },
    ModelMeta {
        model_pattern: "llama-4-maverick",
        display_name: "Llama 4 Maverick",
        context_window: 1_000_000,
        input_price: 0.20,
        output_price: 0.60,
        cache_write_price: None,
        cache_read_price: None,
        supports_reasoning: false,
        reasoning_effort: None,
    },
    ModelMeta {
        model_pattern: "llama-4-scout",
        display_name: "Llama 4 Scout",
        context_window: 512_000,
        input_price: 0.15,
        output_price: 0.40,
        cache_write_price: None,
        cache_read_price: None,
        supports_reasoning: false,
        reasoning_effort: None,
    },
    // ── Mistral ──
    ModelMeta {
        model_pattern: "mistral-large",
        display_name: "Mistral Large",
        context_window: 128_000,
        input_price: 2.00,
        output_price: 6.00,
        cache_write_price: None,
        cache_read_price: None,
        supports_reasoning: false,
        reasoning_effort: None,
    },
    ModelMeta {
        model_pattern: "codestral",
        display_name: "Codestral",
        context_window: 256_000,
        input_price: 0.30,
        output_price: 0.90,
        cache_write_price: None,
        cache_read_price: None,
        supports_reasoning: false,
        reasoning_effort: None,
    },
    // ── Xiaomi MiMo ──
    ModelMeta {
        model_pattern: "mimo-v2.5-pro",
        display_name: "MiMo v2.5 Pro",
        context_window: 128_000,
        input_price: 0.0,
        output_price: 0.0,
        cache_write_price: None,
        cache_read_price: None,
        supports_reasoning: true,
        reasoning_effort: Some("medium"),
    },
    ModelMeta {
        model_pattern: "mimo-v2.5",
        display_name: "MiMo v2.5",
        context_window: 128_000,
        input_price: 0.0,
        output_price: 0.0,
        cache_write_price: None,
        cache_read_price: None,
        supports_reasoning: false,
        reasoning_effort: None,
    },
    ModelMeta {
        model_pattern: "mimo-v2-flash",
        display_name: "MiMo v2 Flash",
        context_window: 128_000,
        input_price: 0.0,
        output_price: 0.0,
        cache_write_price: None,
        cache_read_price: None,
        supports_reasoning: false,
        reasoning_effort: None,
    },
];

/// Look up metadata for a model (fuzzy match: longest pattern that's contained in model name)
pub fn lookup_model(model_name: &str) -> Option<&'static ModelMeta> {
    let lower = model_name.to_lowercase();
    MODEL_CATALOG
        .iter()
        .filter(|m| lower.contains(&m.model_pattern.to_lowercase()))
        .max_by_key(|m| m.model_pattern.len())
}

/// Infer context window from model name heuristics when not in catalog.
/// Returns a reasonable default based on model family patterns.
pub fn infer_context_window(model_name: &str) -> u64 {
    if let Some(meta) = lookup_model(model_name) {
        return meta.context_window;
    }
    let lower = model_name.to_lowercase();
    // Models with known large context
    if lower.contains("gemini") || lower.contains("llama-4") {
        1_000_000
    } else if lower.contains("claude") {
        200_000
    } else if lower.contains("gpt-4") || lower.contains("gpt-5") || lower.contains("qwen") {
        128_000
    } else if lower.contains("deepseek") {
        64_000
    } else {
        // Conservative default
        128_000
    }
}

/// Check if a model likely supports reasoning/thinking based on name
pub fn infer_supports_reasoning(model_name: &str) -> bool {
    if let Some(meta) = lookup_model(model_name) {
        return meta.supports_reasoning;
    }
    let lower = model_name.to_lowercase();
    lower.contains("o3")
        || lower.contains("o4")
        || lower.contains("reasoner")
        || lower.contains("r1")
        || lower.contains("qwq")
        || lower.contains("thinking")
        || lower.contains("opus")
        || lower.contains("sonnet-4")
        || lower.contains("gpt-5")
}

/// Format context window for display
pub fn format_context(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{}M", tokens / 1_000_000)
    } else {
        format!("{}K", tokens / 1_000)
    }
}

/// Format price for display
pub fn format_price(price: f64) -> String {
    if price < 0.01 {
        "<$0.01".into()
    } else if price < 1.0 {
        format!("${:.2}", price)
    } else {
        format!("${:.1}", price)
    }
}
