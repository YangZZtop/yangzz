use super::{Skill, SkillCategory};
use std::path::Path;

/// Load skills from SKILL.md files — merge global (~/.yangzz/skills/) + project
/// (.yangzz/skills/) + single project-root SKILL.md.
/// Also supports directory-based skills: .yangzz/skills/<name>/SKILL.md
/// Project overrides global on name collision.
pub fn load_skills(dir: &Path) -> Vec<Skill> {
    let mut skills: Vec<Skill> = Vec::new();

    // Global skills first (lowest priority)
    load_skills_from_dir(&crate::paths::yangzz_dir().join("skills"), &mut skills);

    // Project-local overrides by name
    let mut project: Vec<Skill> = Vec::new();
    load_skills_from_dir(&dir.join(".yangzz/skills"), &mut project);
    for s in project {
        skills.retain(|ex| !ex.name.eq_ignore_ascii_case(&s.name));
        skills.push(s);
    }

    // Single SKILL.md at project root (treated as project)
    let root_skill = dir.join("SKILL.md");
    if root_skill.exists() {
        if let Some(skill) = parse_skill_file(&root_skill) {
            skills.retain(|ex| !ex.name.eq_ignore_ascii_case(&skill.name));
            skills.push(skill);
        }
    }

    skills
}

/// Load skills from a directory. Supports both:
/// - Flat: skills/review.md
/// - Directory: skills/review/SKILL.md
fn load_skills_from_dir(dir: &Path, out: &mut Vec<Skill>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_file() && path.extension().is_some_and(|e| e == "md") {
            // Flat file: skills/review.md
            if let Some(skill) = parse_skill_file(&path) {
                out.push(skill);
            }
        } else if path.is_dir() {
            // Directory: skills/review/SKILL.md
            let skill_md = path.join("SKILL.md");
            if skill_md.exists() {
                if let Some(skill) = parse_skill_file(&skill_md) {
                    out.push(skill);
                }
            }
        }
    }
}

/// Parse a SKILL.md file with YAML front matter.
///
/// Supported frontmatter fields:
/// - name (string, required) — kebab-case identifier
/// - description (string) — one-line summary
/// - category (string) — workflow / domain / tool
/// - triggers (list of strings) — keywords that activate this skill
/// - allowed-tools (string, comma-separated) — which tools this skill may use
/// - user-invocable (bool) — whether users can explicitly trigger it
fn parse_skill_file(path: &Path) -> Option<Skill> {
    let content = std::fs::read_to_string(path).ok()?;

    // Split YAML front matter from body
    if !content.starts_with("---") {
        // No front matter, use filename as name
        let name = path.file_stem()?.to_string_lossy().to_string();
        return Some(Skill {
            name: name.clone(),
            description: String::new(),
            triggers: vec![name.to_lowercase()],
            body: content,
            category: SkillCategory::Workflow,
            allowed_tools: vec![],
            user_invocable: true,
        });
    }

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return None;
    }

    let front_matter = parts[1].trim();
    let body = parts[2].trim().to_string();

    // Parse frontmatter fields
    let mut name = String::new();
    let mut description = String::new();
    let mut triggers = Vec::new();
    let mut category = SkillCategory::Workflow;
    let mut allowed_tools = Vec::new();
    let mut user_invocable = true;
    let mut in_triggers = false;

    for line in front_matter.lines() {
        let line = line.trim();

        // Detect list continuation
        if line.starts_with("- ") && in_triggers {
            triggers.push(line.strip_prefix("- ").unwrap().trim().trim_matches('"').to_string());
            continue;
        }

        // New field starts — stop list mode
        if !line.starts_with("- ") {
            in_triggers = false;
        }

        if let Some(val) = line.strip_prefix("name:") {
            name = val.trim().trim_matches('"').to_string();
        } else if let Some(val) = line.strip_prefix("description:") {
            description = val.trim().trim_matches('"').to_string();
        } else if line.starts_with("triggers:") {
            in_triggers = true;
            // Check for inline list: triggers: [a, b, c]
            let rest = line.strip_prefix("triggers:").unwrap().trim();
            if rest.starts_with('[') {
                let inner = rest.trim_start_matches('[').trim_end_matches(']');
                for item in inner.split(',') {
                    let t = item.trim().trim_matches('"').trim_matches('\'').to_string();
                    if !t.is_empty() {
                        triggers.push(t);
                    }
                }
                in_triggers = false;
            }
        } else if let Some(val) = line.strip_prefix("category:") {
            category = SkillCategory::from_str(val.trim().trim_matches('"'));
        } else if let Some(val) = line.strip_prefix("allowed-tools:") {
            allowed_tools = val
                .trim()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        } else if let Some(val) = line.strip_prefix("user-invocable:") {
            user_invocable = val.trim().eq_ignore_ascii_case("true");
        }
    }

    if name.is_empty() {
        name = path.file_stem()?.to_string_lossy().to_string();
    }
    if triggers.is_empty() {
        triggers.push(name.to_lowercase());
    }

    Some(Skill {
        name,
        description,
        triggers,
        body,
        category,
        allowed_tools,
        user_invocable,
    })
}
