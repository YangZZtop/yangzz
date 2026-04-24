use super::Skill;
use std::path::Path;

/// Load skills from SKILL.md files — merge global (~/.yangzz/skills/) + project
/// (.yangzz/skills/) + single project-root SKILL.md.
/// Project overrides global on name collision.
pub fn load_skills(dir: &Path) -> Vec<Skill> {
    let mut skills: Vec<Skill> = Vec::new();

    // Global skills first (lowest priority)
    load_md_into(&crate::paths::yangzz_dir().join("skills"), &mut skills);

    // Project-local overrides by name
    let mut project: Vec<Skill> = Vec::new();
    load_md_into(&dir.join(".yangzz/skills"), &mut project);
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

fn load_md_into(dir: &Path, out: &mut Vec<Skill>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                if let Some(skill) = parse_skill_file(&path) {
                    out.push(skill);
                }
            }
        }
    }
}

/// Parse a SKILL.md file with YAML front matter
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
        });
    }

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return None;
    }

    let front_matter = parts[1].trim();
    let body = parts[2].trim().to_string();

    // Simple YAML parsing (name, description, triggers)
    let mut name = String::new();
    let mut description = String::new();
    let mut triggers = Vec::new();

    for line in front_matter.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("name:") {
            name = val.trim().trim_matches('"').to_string();
        } else if let Some(val) = line.strip_prefix("description:") {
            description = val.trim().trim_matches('"').to_string();
        } else if line.starts_with("- ") && !triggers.is_empty() || line.starts_with("triggers:") {
            if let Some(val) = line.strip_prefix("- ") {
                triggers.push(val.trim().trim_matches('"').to_string());
            }
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
    })
}
