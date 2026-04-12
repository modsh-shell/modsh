//! Expander — Variable, glob, and command expansion

use thiserror::Error;

/// Errors that can occur during expansion
#[derive(Error, Debug)]
pub enum ExpandError {
    #[error("undefined variable: {0}")]
    UndefinedVariable(String),
    #[error("invalid parameter expansion: {0}")]
    InvalidParameter(String),
    #[error("command substitution failed: {0}")]
    CommandSubstitution(String),
    #[error("arithmetic expansion error: {0}")]
    ArithmeticError(String),
}

/// Environment for variable expansion
pub struct Environment {
    vars: std::collections::HashMap<String, String>,
}

impl Environment {
    /// Create a new empty environment
    pub fn new() -> Self {
        Self {
            vars: std::collections::HashMap::new(),
        }
    }

    /// Create from the system environment
    pub fn from_system() -> Self {
        let mut vars = std::collections::HashMap::new();
        for (key, value) in std::env::vars() {
            vars.insert(key, value);
        }
        Self { vars }
    }

    /// Get a variable value
    pub fn get(&self, name: &str) -> Option<&String> {
        self.vars.get(name)
    }

    /// Set a variable value
    pub fn set(&mut self, name: String, value: String) {
        self.vars.insert(name, value);
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

/// Expander for shell word expansion
pub struct Expander<'a> {
    env: &'a Environment,
}

impl<'a> Expander<'a> {
    /// Create a new expander with the given environment
    pub fn new(env: &'a Environment) -> Self {
        Self { env }
    }

    /// Expand a word according to POSIX rules
    pub fn expand(&self, word: &str) -> Result<Vec<String>, ExpandError> {
        // TODO: Full POSIX expansion
        // 1. Tilde expansion (~, ~user)
        // 2. Parameter expansion ($VAR, ${VAR}, ${VAR:-default}, etc.)
        // 3. Command substitution ($(cmd), `cmd`)
        // 4. Arithmetic expansion ($((expr)))
        // 5. Word splitting
        // 6. Glob/pathname expansion

        let expanded = self.expand_parameters(word)?;
        let expanded = self.expand_tilde(&expanded);

        // For now, just return as single word (no word splitting)
        Ok(vec![expanded])
    }

    fn expand_parameters(&self, word: &str) -> Result<String, ExpandError> {
        let mut result = String::new();
        let mut chars = word.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '$' {
                // Parameter expansion
                match chars.peek() {
                    Some('{') => {
                        chars.next(); // consume {
                        let name = self.read_braced_name(&mut chars)?;
                        let value = self.expand_braced(&name)?;
                        result.push_str(&value);
                    }
                    Some(&c) if c.is_alphabetic() || c == '_' => {
                        let name = self.read_name(&mut chars);
                        let value = self.env.get(&name).cloned().unwrap_or_default();
                        result.push_str(&value);
                    }
                    Some(&c) if c.is_ascii_digit() || c == '@' || c == '*' || c == '#' || c == '?' || c == '-' || c == '$' || c == '!' => {
                        chars.next(); // consume special var
                        let value = self.expand_special(c);
                        result.push_str(&value);
                    }
                    _ => {
                        result.push('$');
                    }
                }
            } else {
                result.push(ch);
            }
        }

        Ok(result)
    }

    fn read_braced_name(&self, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<String, ExpandError> {
        let mut name = String::new();

        while let Some(&ch) = chars.peek() {
            if ch == '}' {
                chars.next(); // consume }
                return Ok(name);
            }
            if ch == ':' || ch == '-' || ch == '=' || ch == '?' || ch == '+' {
                // Parameter expansion operators - stop here for now
                // TODO: Implement full parameter expansion operators
                return Ok(name);
            }
            name.push(ch);
            chars.next();
        }

        Err(ExpandError::InvalidParameter("unclosed brace".to_string()))
    }

    fn read_name(&self, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
        let mut name = String::new();

        while let Some(&ch) = chars.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                name.push(ch);
                chars.next();
            } else {
                break;
            }
        }

        name
    }

    fn expand_braced(&self, name: &str) -> Result<String, ExpandError> {
        // Check for special syntax like ${VAR:-default}
        if let Some((var_name, suffix)) = name.split_once(':') {
            match suffix.chars().next() {
                Some('-') => {
                    let default = &suffix[1..];
                    Ok(self.env.get(var_name).cloned().unwrap_or_else(|| default.to_string()))
                }
                Some('=') => {
                    // TODO: Set if unset
                    Ok(self.env.get(var_name).cloned().unwrap_or_default())
                }
                Some('?') => {
                    let msg = &suffix[1..];
                    match self.env.get(var_name) {
                        Some(v) => Ok(v.clone()),
                        None => Err(ExpandError::InvalidParameter(
                            msg.to_string()
                        )),
                    }
                }
                Some('+') => {
                    let alternate = &suffix[1..];
                    if self.env.get(var_name).is_some() {
                        Ok(alternate.to_string())
                    } else {
                        Ok(String::new())
                    }
                }
                _ => {
                    Ok(self.env.get(name).cloned().unwrap_or_default())
                }
            }
        } else {
            Ok(self.env.get(name).cloned().unwrap_or_default())
        }
    }

    fn expand_special(&self, ch: char) -> String {
        match ch {
            '0' => std::env::current_exe()
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .unwrap_or_default(),
            '$' => std::process::id().to_string(),
            '?' => "0".to_string(), // TODO: Track exit status
            '#' => "0".to_string(), // TODO: Track positional params
            _ => String::new(),
        }
    }

    fn expand_tilde(&self, word: &str) -> String {
        if word.starts_with("~/") {
            let home = std::env::var("HOME").unwrap_or_default();
            home + &word[1..]
        } else if word.starts_with("~") {
            // ~username expansion - simplified
            // TODO: Look up user's home directory
            word.to_string()
        } else {
            word.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_simple_variable() {
        let mut env = Environment::new();
        env.set("FOO".to_string(), "bar".to_string());

        let expander = Expander::new(&env);
        let result = expander.expand("$FOO").unwrap();
        assert_eq!(result, vec!["bar"]);
    }

    #[test]
    fn test_expand_braced_variable() {
        let mut env = Environment::new();
        env.set("FOO".to_string(), "bar".to_string());

        let expander = Expander::new(&env);
        let result = expander.expand("${FOO}").unwrap();
        assert_eq!(result, vec!["bar"]);
    }

    #[test]
    fn test_expand_default_value() {
        let env = Environment::new();

        let expander = Expander::new(&env);
        let result = expander.expand("${FOO:-default}").unwrap();
        assert_eq!(result, vec!["default"]);
    }

    #[test]
    fn test_expand_with_text() {
        let mut env = Environment::new();
        env.set("USER".to_string(), "alice".to_string());

        let expander = Expander::new(&env);
        let result = expander.expand("Hello, $USER!").unwrap();
        assert_eq!(result, vec!["Hello, alice!"]);
    }
}
