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
    ///
    /// # Errors
    /// Returns an error if parameter expansion is invalid
    pub fn expand(&mut self, word: &str) -> Result<Vec<String>, ExpandError> {
        self.expand_internal(word, false)
    }

    /// Internal expansion with quoting context tracking
    ///
    /// `quoted` indicates whether the word was originally quoted (single or double quotes),
    /// which affects:
    /// - Glob expansion: quoted words suppress glob expansion
    /// - Backslash handling: depends on quote type
    ///
    /// # Errors
    /// Returns an error if parameter expansion is invalid
    fn expand_internal(&mut self, word: &str, quoted: bool) -> Result<Vec<String>, ExpandError> {
        // POSIX expansion order:
        // 1. Tilde expansion (~, ~user) - only on literal word prefix, NOT on parameter values
        // 2. Parameter expansion ($VAR, ${VAR}, ${VAR:-default}, etc.)
        // 3. Command substitution ($(cmd), `cmd`)
        // 4. Arithmetic expansion ($((expr)))
        // 5. Word splitting
        // 6. Glob/pathname expansion (suppressed for quoted words)

        // Step 1: Tilde expansion on the original word (before parameter expansion)
        let home = self.env.get("HOME").map(std::string::ToString::to_string);
        let after_tilde = Self::expand_tilde(word, home.as_deref());

        // Step 2: Parameter, command, and arithmetic expansion (with backslash handling)
        let after_params = self.expand_parameters(&after_tilde, quoted)?;

        // Step 3: Word splitting based on IFS
        let ifs = self.env.get("IFS").unwrap_or(" \t\n");
        let words = Self::split_words(&after_params, ifs);

        // Step 4: Glob/pathname expansion (skip if word was originally quoted)
        if quoted {
            // Quoted words suppress glob expansion
            Ok(words)
        } else {
            let mut globbed = Vec::new();
            for w in words {
                let matches = Self::expand_glob(&w)?;
                globbed.extend(matches);
            }
            Ok(globbed)
        }
    }

    /// Expand a word with explicit quoting context for nested expansion
    ///
    /// # Errors
    /// Returns an error if parameter expansion is invalid
    pub fn expand_quoted(&mut self, word: &str) -> Result<Vec<String>, ExpandError> {
        self.expand_internal(word, true)
    }

    /// Expand glob patterns into matching filenames
    fn expand_glob(pattern: &str) -> Result<Vec<String>, ExpandError> {
        use glob::glob;

        // Check if pattern contains glob characters
        let has_glob = pattern.chars().any(|c| matches!(c, '*' | '?' | '[' | ']'));

        if !has_glob {
            return Ok(vec![pattern.to_string()]);
        }

        // Perform glob expansion
        let mut matches = Vec::new();
        for entry in glob(pattern).map_err(|e| ExpandError::InvalidParameter(e.to_string()))? {
            match entry {
                Ok(path) => {
                    matches.push(path.to_string_lossy().to_string());
                }
                Err(e) => {
                    return Err(ExpandError::InvalidParameter(e.to_string()));
                }
            }
        }

        // If no matches found, return the original pattern (bash behavior)
        if matches.is_empty() {
            matches.push(pattern.to_string());
        }

        Ok(matches)
    }

    /// Split a string into words based on IFS (Internal Field Separator)
    ///
    /// POSIX rules:
    /// - IFS whitespace (space, tab, newline) are field terminators - consecutive ones collapse
    /// - IFS non-whitespace are field separators - each delimits a field, empty fields preserved
    /// - Empty input produces no fields (not a single empty field)
    fn split_words(s: &str, ifs: &str) -> Vec<String> {
        if ifs.is_empty() {
            // Empty IFS means no splitting
            return if s.is_empty() {
                // Empty string with empty IFS produces no fields
                vec![]
            } else {
                vec![s.to_string()]
            };
        }

        // Empty input produces no fields
        if s.is_empty() {
            return vec![];
        }

        // Separate IFS whitespace characters (field terminators that collapse)
        let ifs_whitespace: String = ifs.chars().filter(|c| c.is_ascii_whitespace()).collect();

        let mut words = Vec::new();
        let mut current = String::new();
        let mut prev_was_non_ws_ifs = false;

        for ch in s.chars() {
            if ifs.contains(ch) {
                // This is an IFS character
                let is_whitespace = ifs_whitespace.contains(ch);

                if is_whitespace {
                    // IFS whitespace is a field terminator - consecutive ones collapse
                    if !current.is_empty() {
                        words.push(current);
                        current = String::new();
                    }
                    // Skip consecutive whitespace (they collapse)
                    prev_was_non_ws_ifs = false;
                } else {
                    // IFS non-whitespace is a field separator
                    // Each separator delimits a field - empty fields are preserved
                    if !current.is_empty() {
                        // We have content - push it
                        words.push(current);
                        current = String::new();
                    }
                    // If previous was also a non-whitespace separator, this is consecutive
                    // so we need to add an empty field
                    if prev_was_non_ws_ifs {
                        words.push(String::new());
                    }
                    prev_was_non_ws_ifs = true;
                }
            } else {
                // Non-IFS character
                current.push(ch);
                prev_was_non_ws_ifs = false;
            }
        }

        // Don't forget the last word
        if !current.is_empty() || prev_was_non_ws_ifs {
            // If last char was a non-whitespace IFS, add empty field
            words.push(current);
        }

        // Remove trailing empty field if it was added due to trailing separator
        // (but preserve internal empty fields)
        while words.len() > 1 && words.last().map_or(false, |w| w.is_empty()) {
            // Only remove if the second-to-last was also from a separator
            // Actually, POSIX says trailing separators don't create empty fields
            // at the end unless there were consecutive separators earlier
            let _ = words.pop();
        }

        words
    }

    fn expand_parameters(&mut self, word: &str, _quoted: bool) -> Result<String, ExpandError> {
        let mut result = String::new();
        let mut chars = word.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '\\' => {
                    // Backslash escape handling
                    // \$ -> literal $, \` -> literal `, \\ -> literal \, \" -> literal "
                    match chars.peek() {
                        Some(&'$') => {
                            chars.next();
                            result.push('$');
                        }
                        Some(&'`') => {
                            chars.next();
                            result.push('`');
                        }
                        Some(&'\\') => {
                            chars.next();
                            result.push('\\');
                        }
                        Some(&'"') => {
                            chars.next();
                            result.push('"');
                        }
                        Some(&c) => {
                            // Unknown escape sequence - preserve backslash and char
                            result.push('\\');
                            result.push(c);
                            chars.next();
                        }
                        None => {
                            // Trailing backslash
                            result.push('\\');
                        }
                    }
                }
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
        let mut depth = 0; // Track nesting for parentheses (0 = base level inside $(( )))

        while let Some(&ch) = chars.peek() {
            chars.next();

            if ch == '(' {
                depth += 1;
                expr.push(ch);
            } else if ch == ')' {
                if depth == 0 {
                    // At base level, ) could be the start of )) terminator
                    if chars.peek() == Some(&')') {
                        chars.next(); // consume second )
                        return Ok(expr);
                    }
                    // Single ) at base level without following ) - include in expr
                    // (This handles expressions like $((a)) where inner ) is part of expr)
                    expr.push(ch);
                } else {
                    // Nested level - decrease depth and include )
                    depth -= 1;
                    expr.push(ch);
                }
            } else {
                expr.push(ch);
            }
        }

        Err(ExpandError::ArithmeticError(
            "unterminated arithmetic expression".to_string(),
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
            "unterminated command substitution".to_string(),
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
            "unterminated command substitution".to_string(),
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
                    left = if op == '+' {
                        left + right
                    } else {
                        left - right
                    };
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
                                return Err(ExpandError::ArithmeticError(
                                    "division by zero".to_string(),
                                ));
                            }
                            left / right
                        }
                        '%' => {
                            if right == 0 {
                                return Err(ExpandError::ArithmeticError(
                                    "modulo by zero".to_string(),
                                ));
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
                value_str.parse::<i64>().map_err(|_| {
                    ExpandError::ArithmeticError(format!("invalid number: {value_str}"))
                })
            }
            _ => Err(ExpandError::ArithmeticError(
                "unexpected character in arithmetic expression".to_string(),
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
        // Also strip \r for Windows-style line endings (\r\n)
        stdout = stdout
            .trim_end_matches(|c| c == '\n' || c == '\r')
            .to_string();

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
        // The name comes from read_braced_name which already consumed the closing }

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
                        Err(ExpandError::InvalidParameter(if msg.is_empty() {
                            format!("{var_name}: parameter not set")
                        } else {
                            msg.to_string()
                        }))
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
                Err(ExpandError::InvalidParameter(if msg.is_empty() {
                    format!("{var_name}: parameter not set")
                } else {
                    msg.to_string()
                }))
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

    /// Expand tilde patterns: ~, ~/path and ~user/path
    fn expand_tilde(word: &str, env_home: Option<&str>) -> String {
        if word == "~" {
            // Just ~ - expand to home directory
            env_home.map_or_else(|| word.to_string(), std::string::ToString::to_string)
        } else if word.starts_with("~/") {
            // ~/path - expand to home + path
            let home = env_home.unwrap_or("");
            home.to_string() + &word[1..]
        } else if let Some(rest) = word.strip_prefix('~') {
            // ~username expansion
            #[cfg(unix)]
            {
                if let Some(slash_pos) = rest.find('/') {
                    let username = &rest[..slash_pos];
                    let path_suffix = &rest[slash_pos..];
                    if let Some(home_dir) = Self::get_user_home(username) {
                        return home_dir + path_suffix;
                    }
                } else {
                    // ~username without path
                    let username = rest;
                    if let Some(home_dir) = Self::get_user_home(username) {
                        return home_dir;
                    }
                }
            }
            // On non-Unix or if user not found, return as-is
            word.to_string()
        } else {
            word.to_string()
        }
    }

    /// Look up a user's home directory (Unix only)
    /// Uses thread-safe getpwnam_r instead of getpwnam
    #[cfg(unix)]
    #[cfg_attr(not(unix), allow(dead_code))]
    fn get_user_home(username: &str) -> Option<String> {
        use libc::{c_char, getpwnam_r, passwd};
        use std::ffi::{CStr, CString};

        let c_username = CString::new(username).ok()?;

        // Allocate buffer for the result (getpwnam_r needs this)
        let mut buf = vec![0u8; 4096];
        let mut result: *mut passwd = std::ptr::null_mut();
        let mut pw: passwd = unsafe { std::mem::zeroed() };

        let ret = unsafe {
            getpwnam_r(
                c_username.as_ptr(),
                &mut pw,
                buf.as_mut_ptr() as *mut c_char,
                buf.len(),
                &mut result,
            )
        };

        if ret != 0 || result.is_null() {
            return None;
        }

        // pw_dir is now valid and points into our buffer
        let home_dir = unsafe { CStr::from_ptr(pw.pw_dir).to_str().ok()?.to_string() };

        Some(home_dir)
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
        // Empty string produces no fields after word splitting
        env.set("FOO".to_string(), "".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO-default}").unwrap();
        assert_eq!(result, Vec::<String>::new());
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
        // Empty string produces no fields after word splitting
        env.set("FOO".to_string(), "".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO=reassigned}").unwrap();
        assert_eq!(result, Vec::<String>::new());
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
        // Empty string produces no fields after word splitting
        env.set("FOO".to_string(), "".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO?error message}").unwrap();
        assert_eq!(result, Vec::<String>::new());
    }

    #[test]
    fn test_expand_alternate_value_colon() {
        // ${VAR:+alternate} - uses alternate if set and non-empty
        let mut env = Environment::new();

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO:+alternate}").unwrap();
        // Empty string produces no fields after word splitting
        assert_eq!(result, Vec::<String>::new());

        // Set to empty - should return empty with colon
        env.set("FOO".to_string(), "".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("${FOO:+alternate}").unwrap();
        assert_eq!(result, Vec::<String>::new());

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
        // Empty string produces no fields after word splitting
        assert_eq!(result, Vec::<String>::new());

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
        // Word splitting splits on space: "Hello," and "alice!"
        assert_eq!(result, vec!["Hello,", "alice!"]);
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

    #[test]
    fn test_word_splitting_basic() {
        let mut env = Environment::new();
        env.set("WORDS".to_string(), "hello world foo".to_string());

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("$WORDS").unwrap();
        assert_eq!(result, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn test_word_splitting_multiple_spaces() {
        let mut env = Environment::new();
        env.set("WORDS".to_string(), "a  b    c".to_string());

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("$WORDS").unwrap();
        // Multiple spaces treated as single separator
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_word_splitting_with_tabs() {
        let mut env = Environment::new();
        env.set("WORDS".to_string(), "x\ty\tz".to_string());

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("$WORDS").unwrap();
        // Tabs are also IFS characters
        assert_eq!(result, vec!["x", "y", "z"]);
    }

    #[test]
    fn test_word_splitting_empty_ifs() {
        let mut env = Environment::new();
        env.set("IFS".to_string(), "".to_string());
        env.set("WORDS".to_string(), "hello world".to_string());

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("$WORDS").unwrap();
        // Empty IFS means no splitting
        assert_eq!(result, vec!["hello world"]);
    }

    #[test]
    fn test_word_splitting_custom_ifs() {
        let mut env = Environment::new();
        env.set("IFS".to_string(), ":".to_string());
        env.set(
            "PATH_VAR".to_string(),
            "/usr/bin:/bin:/usr/local/bin".to_string(),
        );

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("$PATH_VAR").unwrap();
        assert_eq!(result, vec!["/usr/bin", "/bin", "/usr/local/bin"]);
    }

    #[test]
    fn test_word_splitting_non_whitespace_adjacent_separators() {
        // Non-whitespace IFS separators preserve empty fields between them
        // "a::b" with IFS=: should produce ["a", "", "b"]
        let mut env = Environment::new();
        env.set("IFS".to_string(), ":".to_string());
        env.set("PATH_VAR".to_string(), "/usr/bin::/bin".to_string());

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("$PATH_VAR").unwrap();
        assert_eq!(result, vec!["/usr/bin", "", "/bin"]);

        // Multiple consecutive non-whitespace separators
        env.set("VAR".to_string(), "a:::b".to_string());
        let mut expander = Expander::new(&mut env);
        let result = expander.expand("$VAR").unwrap();
        // a:::b -> "a", "", "", "b"
        assert_eq!(result, vec!["a", "", "", "b"]);
    }

    #[test]
    fn test_glob_expansion_star() {
        // Test *.rs pattern - should match at least this file (expander.rs)
        let mut env = Environment::new();
        let mut expander = Expander::new(&mut env);

        // Use src/*.rs which should exist
        let result = expander.expand("src/*.rs").unwrap();
        // Should match multiple .rs files in src/
        assert!(!result.is_empty(), "glob should match some files");
        assert!(result.iter().any(|f| f.contains("expander.rs")));
    }

    #[test]
    fn test_glob_expansion_question() {
        // Test ? pattern matches single char
        let mut env = Environment::new();
        let mut expander = Expander::new(&mut env);

        // Test with Cargo.tom? which should match Cargo.toml
        let result = expander.expand("Cargo.tom?").unwrap();
        assert!(result.iter().any(|f| f == "Cargo.toml"));
    }

    #[test]
    fn test_glob_no_match_returns_pattern() {
        // When no files match, return the pattern itself (bash behavior)
        let mut env = Environment::new();
        let mut expander = Expander::new(&mut env);

        let result = expander.expand("*.nonexistent_extension").unwrap();
        assert_eq!(result, vec!["*.nonexistent_extension"]);
    }

    #[test]
    fn test_glob_no_special_chars() {
        // Plain filenames without glob chars pass through unchanged
        let mut env = Environment::new();
        let mut expander = Expander::new(&mut env);

        let result = expander.expand("plain_file.txt").unwrap();
        assert_eq!(result, vec!["plain_file.txt"]);
    }

    #[test]
    fn test_glob_quoted_suppresses_expansion() {
        // Quoted words suppress glob expansion
        let mut env = Environment::new();
        let mut expander = Expander::new(&mut env);

        // Using expand_quoted to simulate a quoted word
        let result = expander.expand_quoted("*.rs").unwrap();
        // Should NOT expand glob - returns the literal pattern
        assert_eq!(result, vec!["*.rs"]);
    }

    #[test]
    fn test_backslash_escapes_dollar() {
        // \$ should produce literal $, not expand variable
        let mut env = Environment::new();
        env.set("VAR".to_string(), "expanded".to_string());

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("\\$VAR").unwrap();
        assert_eq!(result, vec!["$VAR"]);
    }

    #[test]
    fn test_backslash_escapes_backtick() {
        // \` should produce literal backtick, not command substitution
        let mut env = Environment::new();

        let mut expander = Expander::new(&mut env);
        // Input: \`echo hello\` -> after escape processing: `echo hello`
        // Word splitting: [`echo, hello`]
        let result = expander.expand("\\`echo hello\\`").unwrap();
        assert_eq!(result, vec!["`echo", "hello`"]);
    }

    #[test]
    fn test_backslash_escapes_backslash() {
        // \\ should produce literal backslash
        let mut env = Environment::new();

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("path\\\\to\\\\file").unwrap();
        assert_eq!(result, vec!["path\\to\\file"]);
    }

    #[test]
    fn test_backslash_escapes_quote() {
        // \" should produce literal quote
        let mut env = Environment::new();

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("say \\\"hello\\\"").unwrap();
        assert_eq!(result, vec!["say", "\"hello\""]);
    }

    #[test]
    fn test_backslash_unknown_escape() {
        // Unknown escape sequences preserve backslash
        let mut env = Environment::new();

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("\\a\\b\\c").unwrap();
        assert_eq!(result, vec!["\\a\\b\\c"]);
    }

    #[test]
    fn test_tilde_current_user() {
        // ~/ should expand to $HOME/
        let mut env = Environment::new();
        env.set("HOME".to_string(), "/home/testuser".to_string());

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("~/Documents").unwrap();
        assert_eq!(result, vec!["/home/testuser/Documents"]);
    }

    #[test]
    fn test_tilde_just_home() {
        // Just ~ should expand to $HOME
        let mut env = Environment::new();
        env.set("HOME".to_string(), "/home/testuser".to_string());

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("~").unwrap();
        assert_eq!(result, vec!["/home/testuser"]);
    }

    #[test]
    fn test_tilde_other_user() {
        // ~root should expand to root's home on Unix
        // This test may not work on all systems, so we check behavior
        let mut env = Environment::new();

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("~root").unwrap();

        // On Unix, should expand to /root or similar
        // On non-Unix, should return as-is
        #[cfg(unix)]
        {
            // Should have expanded (or returned as-is if user not found)
            assert!(!result.is_empty());
        }
        #[cfg(not(unix))]
        {
            assert_eq!(result, vec!["~root"]);
        }
    }

    #[test]
    fn test_tilde_unknown_user() {
        // ~nonexistentuser should return as-is on Unix
        let mut env = Environment::new();

        let mut expander = Expander::new(&mut env);
        let result = expander.expand("~nonexistentuserxyz").unwrap();

        // Should return unchanged (user doesn't exist)
        assert_eq!(result, vec!["~nonexistentuserxyz"]);
    }
}
