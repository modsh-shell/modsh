//! Prompt engine — Async, configurable prompt rendering

/// Prompt configuration
#[derive(Debug, Clone)]
pub struct PromptConfig {
    /// Prompt template
    pub template: String,
    /// Include git branch
    pub show_git: bool,
    /// Include exit code
    pub show_exit_code: bool,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            template: "[user@host cwd]$ ".to_string(),
            show_git: true,
            show_exit_code: true,
        }
    }
}

/// Prompt engine
pub struct PromptEngine {
    config: PromptConfig,
    last_exit_code: u8,
}

impl PromptEngine {
    /// Create a new prompt engine
    #[must_use]
    pub fn new(config: PromptConfig) -> Self {
        Self {
            config,
            last_exit_code: 0,
        }
    }

    /// Render the prompt
    #[must_use]
    pub fn render(&self) -> String {
        let mut result = self.config.template.clone();

        // Replace placeholders
        result = result.replace("[user]", &whoami::username());
        result = result.replace("[host]", &whoami::hostname());
        result = result.replace("[cwd]", &Self::current_dir());

        if self.config.show_git {
            if let Some(branch) = Self::git_branch() {
                result = result.replace("[git]", &format!("({branch})"));
            } else {
                result = result.replace("[git]", "");
            }
        }

        if self.config.show_exit_code && self.last_exit_code != 0 {
            result = format!("[{}] {}", self.last_exit_code, result);
        }

        result
    }

    /// Set the last exit code
    pub fn set_exit_code(&mut self, code: u8) {
        self.last_exit_code = code;
    }

    fn current_dir() -> String {
        std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "?".to_string())
    }

    fn git_branch() -> Option<String> {
        // Simple git branch detection
        let output = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .output()
            .ok()?;

        if output.status.success() {
            String::from_utf8(output.stdout)
                .ok()
                .map(|s| s.trim().to_string())
        } else {
            None
        }
    }
}

/// Simple whoami module
mod whoami {
    pub fn username() -> String {
        std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "user".to_string())
    }

    pub fn hostname() -> String {
        std::env::var("HOSTNAME")
            .or_else(|_| {
                std::process::Command::new("hostname")
                    .output()
                    .ok()
                    .and_then(|o| String::from_utf8(o.stdout).ok())
                    .map(|s| s.trim().to_string())
                    .ok_or_else(|| "localhost".to_string())
            })
            .unwrap_or_else(|_| "localhost".to_string())
    }
}
