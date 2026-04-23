mod loader;
mod matcher;

pub use loader::load_skills;
pub use matcher::match_skill;

use serde::{Deserialize, Serialize};

/// A Skill is a reusable prompt workflow, loaded from SKILL.md files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub body: String,
}

/// Built-in skills
pub fn builtin_skills() -> Vec<Skill> {
    vec![
        Skill {
            name: "review".into(),
            description: "Code review — analyze code for bugs, style, and improvements".into(),
            triggers: vec!["review".into(), "code review".into(), "/review".into()],
            body: r#"You are performing a code review. Analyze the code for:
1. **Bugs**: Logic errors, off-by-one, null/undefined access, race conditions
2. **Style**: Naming, consistency, idiomatic patterns
3. **Performance**: Unnecessary allocations, N+1 queries, missing indexes
4. **Security**: Injection, auth bypass, data exposure
5. **Maintainability**: Complexity, coupling, missing tests

Be specific. Reference line numbers. Suggest fixes."#.into(),
        },
        Skill {
            name: "debug".into(),
            description: "Debug — systematic root cause analysis".into(),
            triggers: vec!["debug".into(), "investigate".into(), "/debug".into()],
            body: r#"You are debugging an issue. Follow this process:
1. **Reproduce**: Understand the exact steps to reproduce
2. **Isolate**: Narrow down to the specific module/function
3. **Hypothesize**: Form 2-3 hypotheses for root cause
4. **Verify**: Use tools to test each hypothesis
5. **Fix**: Implement the minimal fix for the root cause
6. **Verify**: Confirm the fix resolves the issue

Do NOT guess. Use file_read and grep to gather evidence first."#.into(),
        },
        Skill {
            name: "explain".into(),
            description: "Explain — understand code or concepts in depth".into(),
            triggers: vec!["explain".into(), "what does".into(), "how does".into(), "/explain".into()],
            body: r#"Explain the code or concept clearly:
1. **What it does** — one sentence summary
2. **How it works** — step by step walkthrough
3. **Why it's designed this way** — trade-offs and alternatives
4. **Key details** — edge cases, gotchas, performance characteristics

Read the relevant code first before explaining."#.into(),
        },
    ]
}
