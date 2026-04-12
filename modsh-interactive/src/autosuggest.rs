//! Autosuggestions — Fish-style ghost text from history

/// Autosuggestion engine
pub struct AutosuggestEngine {
    history: Vec<String>,
}

impl AutosuggestEngine {
    /// Create a new autosuggestion engine
    pub fn new() -> Self {
        Self { history: Vec::new() }
    }

    /// Add a command to history
    pub fn add_history(&mut self, command: String) {
        if !command.trim().is_empty() {
            self.history.push(command);
        }
    }

    /// Get a suggestion for the given input
    pub fn suggest(&self, input: &str) -> Option<String> {
        if input.is_empty() {
            return None;
        }

        // Find the most recent command that starts with the input
        self.history
            .iter()
            .rev()
            .find(|cmd| cmd.starts_with(input) && cmd != &input)
            .map(|cmd| cmd[input.len()..].to_string())
    }

    /// Get a full suggestion (including the input prefix)
    pub fn full_suggestion(&self, input: &str) -> Option<String> {
        self.suggest(input).map(|suffix| format!("{}{}", input, suffix))
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.history.clear();
    }

    /// Load history from a vector
    pub fn load_history(&mut self, history: Vec<String>) {
        self.history = history;
    }
}

impl Default for AutosuggestEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggest() {
        let mut engine = AutosuggestEngine::new();
        engine.add_history("cd /home/user".to_string());
        engine.add_history("cd /var/log".to_string());
        engine.add_history("ls -la".to_string());

        // Should suggest the most recent matching command
        assert_eq!(engine.suggest("cd"), Some(" /var/log".to_string()));
        assert_eq!(engine.suggest("ls"), Some(" -la".to_string()));
        assert_eq!(engine.suggest("git"), None);
    }

    #[test]
    fn test_full_suggestion() {
        let mut engine = AutosuggestEngine::new();
        engine.add_history("cd /home/user".to_string());

        assert_eq!(engine.full_suggestion("cd"), Some("cd /home/user".to_string()));
    }

    #[test]
    fn test_no_suggest_exact_match() {
        let mut engine = AutosuggestEngine::new();
        engine.add_history("echo hello".to_string());

        // Should not suggest if the input is already the full command
        assert_eq!(engine.suggest("echo hello"), None);
    }
}
