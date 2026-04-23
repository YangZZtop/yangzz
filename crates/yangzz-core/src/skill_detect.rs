//! Autonomous skill detection: analyze project context and auto-activate relevant skills.
//! Scans for config files, package managers, frameworks, and languages to provide
//! intelligent defaults.

use std::path::Path;
use tracing::info;

/// Detected project characteristics
#[derive(Debug, Clone, Default)]
pub struct ProjectProfile {
    pub languages: Vec<String>,
    pub frameworks: Vec<String>,
    pub package_managers: Vec<String>,
    pub has_tests: bool,
    pub has_ci: bool,
    pub has_docker: bool,
    pub has_git: bool,
    pub project_type: Option<String>,
}

/// Detect project characteristics by scanning the working directory
pub fn detect_project(cwd: &Path) -> ProjectProfile {
    let mut profile = ProjectProfile::default();

    // Languages
    let lang_markers = [
        ("Cargo.toml", "Rust"),
        ("package.json", "JavaScript/TypeScript"),
        ("tsconfig.json", "TypeScript"),
        ("pyproject.toml", "Python"),
        ("requirements.txt", "Python"),
        ("setup.py", "Python"),
        ("go.mod", "Go"),
        ("pom.xml", "Java"),
        ("build.gradle", "Java/Kotlin"),
        ("Gemfile", "Ruby"),
        ("mix.exs", "Elixir"),
        ("Makefile", "C/C++"),
        ("CMakeLists.txt", "C/C++"),
        ("*.csproj", "C#"),
        ("pubspec.yaml", "Dart/Flutter"),
        ("Package.swift", "Swift"),
    ];

    for (marker, lang) in &lang_markers {
        if marker.starts_with('*') {
            // glob pattern - skip for now, check common ones
            continue;
        }
        if cwd.join(marker).exists() {
            profile.languages.push(lang.to_string());
        }
    }

    // Frameworks
    let framework_markers = [
        ("next.config.js", "Next.js"),
        ("next.config.ts", "Next.js"),
        ("next.config.mjs", "Next.js"),
        ("nuxt.config.ts", "Nuxt"),
        ("vite.config.ts", "Vite"),
        ("vite.config.js", "Vite"),
        ("angular.json", "Angular"),
        ("svelte.config.js", "Svelte"),
        ("remix.config.js", "Remix"),
        ("tailwind.config.js", "Tailwind CSS"),
        ("tailwind.config.ts", "Tailwind CSS"),
        ("django", "Django"),
        ("flask", "Flask"),
        ("fastapi", "FastAPI"),
        ("Rocket.toml", "Rocket (Rust)"),
        ("actix", "Actix (Rust)"),
    ];

    for (marker, fw) in &framework_markers {
        if cwd.join(marker).exists() {
            profile.frameworks.push(fw.to_string());
        }
    }

    // Also check package.json for dependencies
    if let Ok(pkg) = std::fs::read_to_string(cwd.join("package.json")) {
        let deps_patterns = [
            ("react", "React"),
            ("vue", "Vue"),
            ("svelte", "Svelte"),
            ("express", "Express"),
            ("nestjs", "NestJS"),
            ("tailwindcss", "Tailwind CSS"),
        ];
        for (pat, fw) in &deps_patterns {
            if pkg.contains(pat) && !profile.frameworks.contains(&fw.to_string()) {
                profile.frameworks.push(fw.to_string());
            }
        }
    }

    // Package managers
    let pm_markers = [
        ("Cargo.lock", "cargo"),
        ("package-lock.json", "npm"),
        ("yarn.lock", "yarn"),
        ("pnpm-lock.yaml", "pnpm"),
        ("bun.lockb", "bun"),
        ("Pipfile.lock", "pipenv"),
        ("poetry.lock", "poetry"),
        ("go.sum", "go mod"),
    ];

    for (marker, pm) in &pm_markers {
        if cwd.join(marker).exists() {
            profile.package_managers.push(pm.to_string());
        }
    }

    // Tests
    profile.has_tests = cwd.join("tests").exists()
        || cwd.join("test").exists()
        || cwd.join("__tests__").exists()
        || cwd.join("spec").exists();

    // CI
    profile.has_ci = cwd.join(".github/workflows").exists()
        || cwd.join(".gitlab-ci.yml").exists()
        || cwd.join(".circleci").exists()
        || cwd.join("Jenkinsfile").exists();

    // Docker
    profile.has_docker = cwd.join("Dockerfile").exists()
        || cwd.join("docker-compose.yml").exists()
        || cwd.join("docker-compose.yaml").exists();

    // Git
    profile.has_git = cwd.join(".git").exists();

    // Project type inference
    if profile.frameworks.iter().any(|f| f.contains("Next") || f.contains("Nuxt") || f.contains("React") || f.contains("Vue")) {
        profile.project_type = Some("Web Frontend".into());
    } else if profile.frameworks.iter().any(|f| f.contains("Express") || f.contains("FastAPI") || f.contains("Django")) {
        profile.project_type = Some("Web Backend".into());
    } else if profile.languages.contains(&"Rust".to_string()) {
        profile.project_type = Some("Rust Project".into());
    } else if profile.languages.contains(&"Python".to_string()) {
        profile.project_type = Some("Python Project".into());
    }

    info!("Project profile: {:?}", profile);
    profile
}

/// Generate system prompt additions based on detected profile
pub fn profile_to_system_hint(profile: &ProjectProfile) -> String {
    let mut hints = Vec::new();

    if !profile.languages.is_empty() {
        hints.push(format!("Languages: {}", profile.languages.join(", ")));
    }
    if !profile.frameworks.is_empty() {
        hints.push(format!("Frameworks: {}", profile.frameworks.join(", ")));
    }
    if !profile.package_managers.is_empty() {
        hints.push(format!("Package managers: {}", profile.package_managers.join(", ")));
    }
    if profile.has_tests {
        hints.push("Has test directory".to_string());
    }
    if profile.has_ci {
        hints.push("Has CI/CD config".to_string());
    }
    if profile.has_docker {
        hints.push("Has Docker config".to_string());
    }
    if let Some(ref pt) = profile.project_type {
        hints.push(format!("Project type: {pt}"));
    }

    if hints.is_empty() {
        return String::new();
    }

    format!("\n--- Auto-detected project context ---\n{}", hints.join("\n"))
}
