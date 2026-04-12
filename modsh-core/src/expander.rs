//! Expander — Variable, glob, and command expansion

use thiserror::Error;

/// Errors that can occur during expansion
#[derive(Error, Debug)]
pub enum ExpandError {
    /// Undefined variable referenced
    #[error("undefined variable: {0}")]
    UndefinedVariable(String),
    /// Invalid parameter expansion syntax
    #[error("invalid parameter expansion: {0}")]
    InvalidParameter(String),
    /// Command substitution failed
    #[error("command substitution failed: {0}")]
    CommandSubstitution(String),
    /// Arithmetic expansion error
    #[error("arithmetic expansion error: {0}")]
    ArithmeticError(String),
}

/// Environment for variable expansion
pub struct Environment {
    vars: std::collections::HashMap<String, String>,
}

impl Environment {
    /// Create a new empty environment
    #[must_use]
    pub fn new() -> Self {
        Self {
            vars: std::collections::HashMap::new(),
        }
    }

    /// Create from the system environment
    #[must_use]
    pub fn from_system() -> Self {
        let mut vars = std::collections::HashMap::new();
        for (key, value) in std::env::vars() {
            vars.insert(key, value);
        }
        Self { vars }
    }

    /// Get a variable value
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.vars.get(name).map(String::as_str)
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
    env: &'a mut Environment,
}

impl<'a> Expander<'a> {
    /// Create a new expander with the given environment
    pub fn new(env: &'a mut Environment) -> Self {
        Self { env }
    }

    /// Expand a word according to POSIX rules
    /// Expand a word with variable and special character expansion
    ///
    /// # Errors
    /// Returns an error if parameter expansion is invalid
    pub fn expand(&mut self, word: &str) -> Result<Vec<String>, ExpandError> {
        // TODO: Full POSIX expansion
        // 1. Tilde expansion (~, ~user)
        // 2. Parameter expansion ($VAR, ${VAR}, ${VAR:-default}, etc.)
        // 3. Command substitution ($(cmd), `cmd`)
        // 4. Arithmetic expansion ($((expr)))
        // 5. Word splitting
        // 6. Glob/pathname expansion

        let expanded = self.expand_parameters(word)?;
        let expanded = Self::expand_tilde(&expanded);

        // For now, just return as single word (no word splitting)
        Ok(vec![expanded])
    }

    fn expand_parameters(&mut self, word: &str) -> Result<String, ExpandError> {
        let mut result = String::new();
        let mut chars = word.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '$' => {
                    // Parameter expansion, command substitution $(...), or arithmetic $((...))
                    match chars.peek() {
                        Some('{') => {
                            chars.next(); // consume {
                            let name = Self::read_braced_name(&mut chars)?;
                            let value = self.expand_braced(&name)?;
                            result.push_str(&value);
                        }
                        Some(&'(') => {
                            chars.next(); // consume (
                            // Check for arithmetic expansion $((...))
                            if chars.peek() == Some(&'(') {
                                chars.next(); // consume second (
                                let expr = Self::read_arithmetic_expression(&mut chars)?;
                                let value = self.evaluate_arithmetic(&expr)?;
                                result.push_str(&value.to_string());
                            } else {
                                // Command substitution $(...)
                                let cmd = Self::read_command_substitution_paren(&mut chars)?;
                                let output = Self::execute_command_substitution(&cmd)?;
                                result.push_str(&output);
                            }
                        }
                        Some(&c) if c.is_alphabetic() || c == '_' => {
                            let name = Self::read_name(&mut chars);
                            let value = self.env.get(&name).unwrap_or_default();
                            result.push_str(value);
                        }
                        Some(&c)
                            if c.is_ascii_digit()
                                || c == '@'
                                || c == '*'
                                || c == '#'
                                || c == '?'
                                || c == '-'
                                || c == '$'
                                || c == '!' =>
                        {
                            chars.next(); // consume special var
                            let value = Self::expand_special(c);
                            result.push_str(&value);
                        }
                        _ => {
                            result.push('$');
                        }
                    }
                }
                '`' => {
                    // Backtick command substitution
                    let cmd = Self::read_command_substitution(&mut chars, '`')?;
                    let output = Self::execute_command_substitution(&cmd)?;
                    result.push_str(&output);
                }
                _ => {
                    result.push(ch);
                }
            }
        }

        Ok(result)
    }

    /// Read arithmetic expression content until ))
    fn read_arithmetic_expression(
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    ) -> Result<String, ExpandError> {
        let mut expr = String::new();
        let mut depth = 1; // Track nesting for parentheses

        while let Some(&ch) = chars.peek() {
            chars.next();

            if ch == '(' {
                depth += 1;
                expr.push(ch);
            } else if ch == ')' {
                depth -= 1;
                if depth == 0 {
                    // Check for second )
                    if chars.peek() == Some(&')') {
                        chars.next(); // consume second )
                        return Ok(expr);
                    }
                    // Single ) in arithmetic context - keep looking
                    depth = 1;
                }
                expr.push(ch);
            } else {
                expr.push(ch);
            }
        }

        Err(ExpandError::ArithmeticError(
            "unterminated arithmetic expression".to_string()
        ))
    }

    /// Read command substitution content for $(...)
    fn read_command_substitution_paren(
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    ) -> Result<String, ExpandError> {
        let mut cmd = String::new();
        let mut depth = 1; // Track nesting for parentheses

        while let Some(&ch) = chars.peek() {
            chars.next();

            if ch == '(' {
                depth += 1;
                cmd.push(ch);
            } else if ch == ')' {
                depth -= 1;
                if depth == 0 {
                    return Ok(cmd);
                }
                cmd.push(ch);
            } else {
                cmd.push(ch);
            }
        }

        Err(ExpandError::CommandSubstitution(
            "unterminated command substitution".to_string()
        ))
    }

    /// Read command substitution content until terminator (backtick for `...`)
    fn read_command_substitution(
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
        terminator: char,
    ) -> Result<String, ExpandError> {
        let mut cmd = String::new();

        while let Some(&ch) = chars.peek() {
            chars.next();

            if ch == terminator {
                return Ok(cmd);
            }
            cmd.push(ch);
        }

        Err(ExpandError::CommandSubstitution(
            "unterminated command substitution".to_string()
        ))
    }

    /// Evaluate an arithmetic expression
    fn evaluate_arithmetic(&self, expr: &str) -> Result<i64, ExpandError> {
        // Simple recursive descent parser for arithmetic expressions
        let mut chars = expr.chars().peekable();
        self.parse_arithmetic_expr(&mut chars)
    }

    /// Parse arithmetic expression (handles +, -)
    fn parse_arithmetic_expr(
        &self,
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    ) -> Result<i64, ExpandError> {
        let mut left = self.parse_arithmetic_term(chars)?;

        loop {
            // Skip whitespace
            while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                chars.next();
            }

            match chars.peek() {
                Some(&op) if op == '+' || op == '-' => {
                    chars.next();
                    let right = self.parse_arithmetic_term(chars)?;
                    left = if op == '+' { left + right } else { left - right };
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse arithmetic term (handles *, /, %)
    fn parse_arithmetic_term(
        &self,
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    ) -> Result<i64, ExpandError> {
        let mut left = self.parse_arithmetic_factor(chars)?;

        loop {
            // Skip whitespace
            while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                chars.next();
            }

            match chars.peek() {
                Some(&op) if op == '*' || op == '/' || op == '%' => {
                    chars.next();
                    let right = self.parse_arithmetic_factor(chars)?;
                    left = match op {
                        '*' => left * right,
                        '/' => {
                            if right == 0 {
                                return Err(ExpandError::ArithmeticError("division by zero".to_string()));
                            }
                            left / right
                        }
                        '%' => {
                            if right == 0 {
                                return Err(ExpandError::ArithmeticError("modulo by zero".to_string()));
                            }
                            left % right
                        }
                        _ => unreachable!(),
                    };
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse arithmetic factor (handles numbers, variables, parentheses, unary +/-)
    fn parse_arithmetic_factor(
        &self,
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    ) -> Result<i64, ExpandError> {
        // Skip whitespace
        while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
            chars.next();
        }

        match chars.peek() {
            Some(&'(') => {
                chars.next(); // consume (
                let value = self.parse_arithmetic_expr(chars)?;
                // Expect )
                if chars.peek() == Some(&')') {
                    chars.next();
                }
                Ok(value)
            }
            Some(&'+') => {
                chars.next(); // unary plus
                self.parse_arithmetic_factor(chars)
            }
            Some(&'-') => {
                chars.next(); // unary minus
                let value = self.parse_arithmetic_factor(chars)?;
                Ok(-value)
            }
            Some(&c) if c.is_ascii_digit() => {
                let mut num = 0i64;
                while let Some(&ch) = chars.peek() {
                    if ch.is_ascii_digit() {
                        chars.next();
                        num = num * 10 + (ch as i64 - '0' as i64);
                    } else {
                        break;
                    }
                }
                Ok(num)
            }
            Some(&c) if c.is_alphabetic() || c == '_' => {
                // Variable reference in arithmetic context
                let mut name = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_alphanumeric() || ch == '_' {
                        chars.next();
                        name.push(ch);
                    } else {
                        break;
                    }
                }
                // Look up variable and parse as number
                let value_str = self.env.get(&name).unwrap_or("0");
                value_str
                    .parse::<i64>()
                    .map_err(|_| ExpandError::ArithmeticError(format!("invalid number: {value_str}")))
            }
            _ => Err(ExpandError::ArithmeticError(
                "unexpected character in arithmetic expression".to_string()
            )),
        }
    }

    /// Execute a command substitution and return its output
    fn execute_command_substitution(cmd: &str) -> Result<String, ExpandError> {
        // For now, use a simple approach with std::process::Command
        // This runs the command through the system shell
        // TODO: In the future, integrate with our own parser/executor for proper execution
        
        use std::process::{Command, Stdio};
        
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", cmd])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
        } else {
            Command::new("sh")
                .args(["-c", cmd])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
        }
        .map_err(|e| ExpandError::CommandSubstitution(e.to_string()))?;

        if !output.status.success() {
            // POSIX: Failed command substitution doesn't necessarily fail expansion
            // The stderr is preserved but expansion uses stdout (which may be empty)
        }

        let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
        
        // POSIX: Strip trailing newlines from command output
        while stdout.ends_with('\n') {
            stdout.pop();
        }
        
        Ok(stdout)
    }

    fn read_braced_name(
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    ) -> Result<String, ExpandError> {
        let mut name = String::new();

        while let Some(&ch) = chars.peek() {
            if ch == '}' {
                chars.next(); // consume }
                return Ok(name);
            }
            name.push(ch);
            chars.next();
        }

        Err(ExpandError::InvalidParameter("unclosed brace".to_string()))
    }

    fn read_name(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
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

    fn expand_braced(&mut self, name: &str) -> Result<String, ExpandError> {
        // Check for special syntax like ${VAR:-default}
        // The name may include trailing } which we need to strip
        let name = name.strip_suffix('}').unwrap_or(name);

        // Check for colon-prefix operators (:-, :=, :?, :+)
        // These treat empty string as unset
        if let Some((var_name, suffix)) = name.split_once(':') {
            match suffix.chars().next() {
                Some('-') => {
                    let default = &suffix[1..];
                    let is_unset_or_empty = self.env.get(var_name).map_or(true, str::is_empty);
                    if is_unset_or_empty {
                        Ok(default.to_string())
                    } else {
                        Ok(self.env.get(var_name).unwrap().to_string())
                    }
                }
                Some('=') => {
                    let default = &suffix[1..];
                    let is_unset_or_empty = self.env.get(var_name).map_or(true, str::is_empty);
                    if is_unset_or_empty {
                        self.env.set(var_name.to_string(), default.to_string());
                        Ok(default.to_string())
                    } else {
                        Ok(self.env.get(var_name).unwrap().to_string())
                    }
                }
                Some('?') => {
                    let msg = &suffix[1..];
                    let is_unset_or_empty = self.env.get(var_name).map_or(true, str::is_empty);
                    if is_unset_or_empty {
                        Err(ExpandError::InvalidParameter(
                            if msg.is_empty() {
                                format!("{var_name}: parameter not set")
                            } else {
                                msg.to_string()
                            }
                        ))
                    } else {
                        Ok(self.env.get(var_name).unwrap().to_string())
                    }
                }
                Some('+') => {
                    let alternate = &suffix[1..];
                    let is_unset_or_empty = self.env.get(var_name).map_or(true, str::is_empty);
                    if is_unset_or_empty {
                        Ok(String::new())
                    } else {
                        Ok(alternate.to_string())
                    }
                }
                _ => Ok(self
                    .env
                    .get(name)
                    .map(ToString::to_string)
                    .unwrap_or_default()),
            }
        } else if let Some((var_name, suffix)) = name.split_once('=') {
            // Non-colon := sets only if truly unset (empty is considered set)
            let default = suffix;
            if self.env.get(var_name).is_none() {
                self.env.set(var_name.to_string(), default.to_string());
                Ok(default.to_string())
            } else {
                Ok(self.env.get(var_name).unwrap().to_string())
            }
        } else if let Some((var_name, suffix)) = name.split_once('-') {
            // Non-colon :- uses default only if truly unset
            let default = suffix;
            if self.env.get(var_name).is_none() {
                Ok(default.to_string())
            } else {
                Ok(self.env.get(var_name).unwrap().to_string())
            }
        } else if let Some((var_name, suffix)) = name.split_once('+') {
            // Non-colon :+ uses alternate only if truly set
            let alternate = suffix;
            if self.env.get(var_name).is_some() {
                Ok(alternate.to_string())
            } else {
                Ok(String::new())
            }
        } else if let Some((var_name, suffix)) = name.split_once('?') {
            // Non-colon :? errors only if truly unset
            let msg = suffix;
            if self.env.get(var_name).is_none() {
                Err(ExpandError::InvalidParameter(
                    if msg.is_empty() {
                        format!("{var_name}: parameter not set")
                    } else {
                        msg.to_string()
                    }
                ))
            } else {
                Ok(self.env.get(var_name).unwrap().to_string())
            }
        } else {
            Ok(self
                .env
                .get(name)
                .map(ToString::to_string)
                .unwrap_or_default())
        }
    }

    fn expand_special(ch: char) -> String {
        match ch {
            '0' => std::env::current_exe()
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .unwrap_or_default(),
            '$' => std::process::id().to_string(),
            '?' | '#' => "0".to_string(), // TODO: Track exit status / positional params
            _ => String::new(),
        }
    }

    fn expand_tilde(word: &str) -> String {
        if word.starts_with("~/") {
            let home = std::env::var("HOME").unwrap_or_default();
            home + &word[1..]
        } else if word.starts_with('~') {
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
    fn test_expand_command_substitution_dollar_paren() {
        let mut env = Environment::new();
        let mut expander = Expander::new(&mut env);
        
        // Test simple echo
        let result = expander.expand("$(echo hello)").unwrap();
        assert_eq!(result, vec!["hello"]);
    }

    #[test]
    fn test_expand_command_substitution_backtick() {
        let mut env = Environment::new();
        let mut expander = Expander::new(&mut env);
        
        // Test simple echo with backticks
        let result = expander.expand("`echo world`").unwrap();
        assert_eq!(result, vec!["world"]);
    }

    #[test]
    fn test_expand_command_substitution_in_word() {
        let mut env = Environment::new();
        let mut expander = Expander::new(&mut env);
        
        // Command substitution embedded in a word
        let result = expander.expand("prefix-$(echo middle)-suffix").unwrap();
        assert_eq!(result, vec!["prefix-middle-suffix"]);
    }

    #[test]
    fn test_expand_arithmetic_simple() {
        let mut env = Environment::new();
        let mut expander = Expander::new(&mut env);
        
        // Basic arithmetic
        assert_eq!(expander.expand("$((1 + 2))").unwrap(), vec!["3"]);
        assert_eq!(expander.expand("$((10 - 3))").unwrap(), vec!["7"]);
        assert_eq!(expander.expand("$((4 * 5))").unwrap(), vec!["20"]);
        assert_eq!(expander.expand("$((20 / 4))").unwrap(), vec!["5"]);
        assert_eq!(expander.expand("$((17 % 5))").unwrap(), vec!["2"]);
    }

    #[test]
    fn test_expand_arithmetic_complex() {
        let mut env = Environment::new();
        let mut expander = Expander::new(&mut env);
        
        // Complex expressions with parentheses
        assert_eq!(expander.expand("$((2 + 3 * 4))").unwrap(), vec!["14"]); // Precedence
        assert_eq!(expander.expand("$(((2 + 3) * 4))").unwrap(), vec!["20"]); // Parentheses
        assert_eq!(expander.expand("$((-5 + 3))").unwrap(), vec!["-2"]); // Unary minus
        assert_eq!(expander.expand("$((+7))").unwrap(), vec!["7"]); // Unary plus
    }

    #[test]
    fn test_expand_arithmetic_with_variable() {
        let mut env = Environment::new();
        env.set("X".to_string(), "10".to_string());
        env.set("Y".to_string(), "3".to_string());
        
        let mut expander = Expander::new(&mut env);
        
        // Variables in arithmetic
        assert_eq!(expander.expand("$((X + Y))").unwrap(), vec!["13"]);
        assert_eq!(expander.expand("$((X * 2))").unwrap(), vec!["20"]);
    }

    #[test]
    fn test_expand_arithmetic_in_word() {
        let mut env = Environment::new();
        let mut expander = Expander::new(&mut env);
        
        // Arithmetic embedded in a word
        let result = expander.expand("count_$((5 + 1))_items").unwrap();
        assert_eq!(result, vec!["count_6_items"]);
    }

    #[test]
    fn test_expand_simple_variable() {
        let mut env = Environment::new();
        env.set("FOO".to_string(), "bar".to_string());

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("$FOO").unwrap();
        assert_eq!(result, vec!["bar"]);
    }

    #[test]
    fn test_expand_braced_variable() {
        let mut env = Environment::new();
        env.set("FOO".to_string(), "bar".to_string());

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO}").unwrap();
        assert_eq!(result, vec!["bar"]);
    }

    #[test]
    fn test_expand_default_value_colon() {
        // ${VAR:-default} - colon treats empty as unset
        let mut env = Environment::new();

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO:-default}").unwrap();
        assert_eq!(result, vec!["default"]);

        // Set to empty - should still use default with colon
        env.set("FOO".to_string(), "".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO:-default}").unwrap();
        assert_eq!(result, vec!["default"]);

        // Set to value - should use value
        env.set("FOO".to_string(), "set_value".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO:-default}").unwrap();
        assert_eq!(result, vec!["set_value"]);
    }

    #[test]
    fn test_expand_default_value_no_colon() {
        // ${VAR-default} - no colon, empty is considered set
        let mut env = Environment::new();

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO-default}").unwrap();
        assert_eq!(result, vec!["default"]);

        // Set to empty - should use empty (not default) without colon
        env.set("FOO".to_string(), "".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO-default}").unwrap();
        assert_eq!(result, vec![""]);
    }

    #[test]
    fn test_expand_assign_default_colon() {
        // ${VAR:=default} - assigns if unset or empty
        let mut env = Environment::new();

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO:=assigned}").unwrap();
        assert_eq!(result, vec!["assigned"]);
        assert_eq!(env.get("FOO"), Some("assigned"));

        // Set to empty - should assign with colon
        env.set("FOO".to_string(), "".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO:=reassigned}").unwrap();
        assert_eq!(result, vec!["reassigned"]);
        assert_eq!(env.get("FOO"), Some("reassigned"));

        // Set to value - should not change
        env.set("FOO".to_string(), "existing".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO:=new}").unwrap();
        assert_eq!(result, vec!["existing"]);
        assert_eq!(env.get("FOO"), Some("existing"));
    }

    #[test]
    fn test_expand_assign_default_no_colon() {
        // ${VAR=default} - assigns only if unset (empty is set)
        let mut env = Environment::new();

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO=assigned}").unwrap();
        assert_eq!(result, vec!["assigned"]);
        assert_eq!(env.get("FOO"), Some("assigned"));

        // Set to empty - should NOT assign without colon
        env.set("FOO".to_string(), "".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO=reassigned}").unwrap();
        assert_eq!(result, vec![""]);
        assert_eq!(env.get("FOO"), Some(""));
    }

    #[test]
    fn test_expand_error_if_unset_colon() {
        // ${VAR:?message} - errors if unset or empty
        let mut env = Environment::new();

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO:?error message}");
        assert!(result.is_err());

        // Set to empty - should error with colon
        env.set("FOO".to_string(), "".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO:?error message}");
        assert!(result.is_err());

        // Set to value - should return value
        env.set("FOO".to_string(), "value".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO:?error message}").unwrap();
        assert_eq!(result, vec!["value"]);
    }

    #[test]
    fn test_expand_error_if_unset_no_colon() {
        // ${VAR?message} - errors only if unset (empty is ok)
        let mut env = Environment::new();

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO?error message}");
        assert!(result.is_err());

        // Set to empty - should NOT error without colon
        env.set("FOO".to_string(), "".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO?error message}").unwrap();
        assert_eq!(result, vec![""]);
    }

    #[test]
    fn test_expand_alternate_value_colon() {
        // ${VAR:+alternate} - uses alternate if set and non-empty
        let mut env = Environment::new();

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO:+alternate}").unwrap();
        assert_eq!(result, vec![""]);

        // Set to empty - should return empty with colon
        env.set("FOO".to_string(), "".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO:+alternate}").unwrap();
        assert_eq!(result, vec![""]);

        // Set to value - should return alternate
        env.set("FOO".to_string(), "value".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO:+alternate}").unwrap();
        assert_eq!(result, vec!["alternate"]);
    }

    #[test]
    fn test_expand_alternate_value_no_colon() {
        // ${VAR+alternate} - uses alternate if set (even if empty)
        let mut env = Environment::new();

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO+alternate}").unwrap();
        assert_eq!(result, vec![""]);

        // Set to empty - should return alternate without colon
        env.set("FOO".to_string(), "".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO+alternate}").unwrap();
        assert_eq!(result, vec!["alternate"]);
    }

    #[test]
    fn test_expand_with_text() {
        let mut env = Environment::new();
        env.set("USER".to_string(), "alice".to_string());

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("Hello, $USER!").unwrap();
        assert_eq!(result, vec!["Hello, alice!"]);
    }

    #[test]
    fn test_expand_default_preserves_existing() {
        // ${VAR:-default} should NOT modify the environment
        let mut env = Environment::new();
        env.set("FOO".to_string(), "".to_string());

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO:-default}").unwrap();
        assert_eq!(result, vec!["default"]);
        // FOO should still be empty (not set to default)
        assert_eq!(env.get("FOO"), Some(""));
    }
}
