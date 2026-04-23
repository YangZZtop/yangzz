use super::Skill;

/// Match user input against available skills
/// Returns the matched skill if any trigger matches
pub fn match_skill<'a>(input: &str, skills: &'a [Skill]) -> Option<&'a Skill> {
    let input_lower = input.to_lowercase();

    // Exact command match first (e.g. "/review")
    for skill in skills {
        for trigger in &skill.triggers {
            if trigger.starts_with('/') && input_lower.starts_with(&trigger.to_lowercase()) {
                return Some(skill);
            }
        }
    }

    // Keyword match — input starts with trigger word
    for skill in skills {
        for trigger in &skill.triggers {
            if !trigger.starts_with('/') && input_lower.starts_with(&trigger.to_lowercase()) {
                return Some(skill);
            }
        }
    }

    None
}
