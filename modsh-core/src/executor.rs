//! Executor — Forks/execs commands, manages pipes

use crate::parser::{Command, RedirectKind, SimpleCommand};
use thiserror::Error;

/// Errors that can occur during execution
#[derive(Error, Debug)]
pub enum ExecError {
    /// Command not found in PATH
    #[error("command not found: {0}")]
    CommandNotFound(String),
    /// IO error during execution
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Invalid redirect specification
    #[error("invalid redirect: {0}")]
    InvalidRedirect(String),
    /// Non-zero exit status
    #[error("exit status: {0}")]
    ExitStatus(i32),
    /// Builtin command error
    #[error("builtin error: {0}")]
    Builtin(#[from] crate::builtins::BuiltinError),
}

/// Result of command execution
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExitStatus {
    /// The exit code (0-255, where 0 is success)
    pub code: u8,
    /// Whether the process was signaled
    pub signaled: bool,
}

impl ExitStatus {
    /// Success status
    pub const SUCCESS: Self = Self {
        code: 0,
        signaled: false,
    };

    /// Create from a process exit status
    #[must_use]
    pub fn from_process(status: std::process::ExitStatus) -> Self {
        if let Some(code) = status.code() {
            Self {
                code: u8::try_from(code).unwrap_or(255),
                signaled: false,
            }
        } else {
            Self {
                code: 1,
                signaled: true,
            }
        }
    }

    /// Check if the command succeeded
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == 0 && !self.signaled
    }
}

/// Executor for shell commands
pub struct Executor {
    /// Environment variables
    pub env: std::collections::HashMap<String, String>,
    /// Current working directory
    pub cwd: std::path::PathBuf,
}

impl Executor {
    /// Create a new executor
    #[must_use]
    pub fn new() -> Self {
        Self {
            env: std::env::vars().collect(),
            cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/")),
        }
    }

    /// Execute a command AST
    ///
    /// # Errors
    /// Returns an error if the command cannot be found or execution fails
    pub fn execute(&mut self, cmd: &Command) -> Result<ExitStatus, ExecError> {
        match cmd {
            Command::Simple(s) => self.execute_simple(s),
            Command::Pipeline(commands) => self.execute_pipeline(commands),
            Command::And(left, right) => {
                let status = self.execute(left)?;
                if status.success() {
                    self.execute(right)
                } else {
                    Ok(status)
                }
            }
            Command::Or(left, right) => {
                let status = self.execute(left)?;
                if status.success() {
                    Ok(status)
                } else {
                    self.execute(right)
                }
            }
            Command::List(left, right) => {
                self.execute(left)?;
                self.execute(right)
            }
            Command::Background(cmd) => {
                // TODO: Fork and run in background
                self.execute(cmd)
            }
            Command::Subshell(cmd) => {
                // TODO: Fork and execute in subshell
                self.execute(cmd)
            }
            Command::Group(cmd) => self.execute(cmd),
            Command::If(if_clause) => self.execute_if(if_clause),
            Command::For(for_loop) => self.execute_for(for_loop),
            Command::While(while_loop) => self.execute_while(while_loop),
            Command::Case(case_stmt) => self.execute_case(case_stmt),
        }
    }

    fn execute_if(&mut self, if_clause: &crate::parser::IfClause) -> Result<ExitStatus, ExecError> {
        let cond_status = self.execute(&if_clause.condition)?;
        if cond_status.success() {
            self.execute(&if_clause.then_branch)
        } else {
            for (elif_cond, elif_then) in &if_clause.elif_branches {
                let elif_status = self.execute(elif_cond)?;
                if elif_status.success() {
                    return self.execute(elif_then);
                }
            }
            if let Some(else_branch) = &if_clause.else_branch {
                self.execute(else_branch)
            } else {
                Ok(ExitStatus::SUCCESS) // No else, no match
            }
        }
    }

    fn execute_for(&mut self, for_loop: &crate::parser::ForLoop) -> Result<ExitStatus, ExecError> {
        let words = if for_loop.words.is_empty() {
            // TODO: Get positional parameters "$@"
            vec![]
        } else {
            for_loop.words.clone()
        };

        let mut last_status = ExitStatus::SUCCESS;
        for word in words {
            // TODO: Set loop variable in environment
            let _ = word;
            last_status = self.execute(&for_loop.body)?;
        }
        Ok(last_status)
    }

    fn execute_while(&mut self, while_loop: &crate::parser::WhileLoop) -> Result<ExitStatus, ExecError> {
        let mut last_status = ExitStatus::SUCCESS;
        loop {
            let cond_status = self.execute(&while_loop.condition)?;
            if !cond_status.success() {
                break;
            }
            last_status = self.execute(&while_loop.body)?;
        }
        Ok(last_status)
    }

    fn execute_case(&mut self, case_stmt: &crate::parser::CaseStatement) -> Result<ExitStatus, ExecError> {
        // TODO: Pattern matching against case_stmt.word
        let mut last_status = ExitStatus::SUCCESS;
        for (_patterns, body) in &case_stmt.clauses {
            // TODO: Check if word matches any pattern
            last_status = self.execute(body)?;
            // TODO: Break after first match (or continue for ;&)
        }
        Ok(last_status)
    }

    fn execute_simple(&mut self, cmd: &SimpleCommand) -> Result<ExitStatus, ExecError> {
        if cmd.words.is_empty() {
            // TODO: Handle redirects only
            return Ok(ExitStatus::SUCCESS);
        }

        let program = &cmd.words[0];
        let args: Vec<&str> = cmd.words[1..].iter().map(String::as_str).collect();

        // Check if it's a builtin
        if let Some(builtin) = crate::builtins::get_builtin(program) {
            return Ok(builtin(&args, &mut self.env)?);
        }

        // Search in PATH
        let program_path = self.find_in_path(program)?;

        // Set up redirects
        let mut stdin = std::process::Stdio::inherit();
        let mut stdout = std::process::Stdio::inherit();
        let mut stderr = std::process::Stdio::inherit();

        for redirect in &cmd.redirects {
            match (&redirect.fd, &redirect.kind) {
                (None | Some(0), RedirectKind::Input) => {
                    stdin = std::process::Stdio::from(std::fs::File::open(&redirect.target)?);
                }
                (None | Some(1), RedirectKind::Output) => {
                    stdout = std::process::Stdio::from(std::fs::File::create(&redirect.target)?);
                }
                (Some(2), RedirectKind::Output) => {
                    stderr = std::process::Stdio::from(std::fs::File::create(&redirect.target)?);
                }
                (None | Some(1), RedirectKind::Append) => {
                    stdout = std::process::Stdio::from(
                        std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&redirect.target)?,
                    );
                }
                _ => {
                    // TODO: Handle other redirect types
                }
            }
        }

        let mut command = std::process::Command::new(&program_path);
        command
            .args(&args)
            .current_dir(&self.cwd)
            .stdin(stdin)
            .stdout(stdout)
            .stderr(stderr);

        // Set environment
        command.env_clear();
        for (k, v) in &self.env {
            command.env(k, v);
        }

        let status = command.spawn()?.wait()?;
        Ok(ExitStatus::from_process(status))
    }

    fn execute_pipeline(&mut self, commands: &[Command]) -> Result<ExitStatus, ExecError> {
        if commands.is_empty() {
            return Ok(ExitStatus::SUCCESS);
        }

        if commands.len() == 1 {
            return self.execute(&commands[0]);
        }

        // For simplicity, we'll just use Command::pipeline()
        // TODO: Proper pipeline implementation with individual process control
        let mut last_stdout: Option<std::process::ChildStdout> = None;
        let mut children: Vec<std::process::Child> = Vec::new();

        for (i, cmd) in commands.iter().enumerate() {
            let is_first = i == 0;
            let is_last = i == commands.len() - 1;

            let program = match cmd {
                Command::Simple(s) if !s.words.is_empty() => &s.words[0],
                _ => {
                    return Err(ExecError::CommandNotFound(
                        "invalid pipeline command".to_string(),
                    ))
                }
            };

            let args: Vec<&str> = match cmd {
                Command::Simple(s) => s.words[1..].iter().map(String::as_str).collect(),
                _ => vec![],
            };

            let program_path = self.find_in_path(program)?;

            let mut command = std::process::Command::new(&program_path);
            command.args(&args).current_dir(&self.cwd);

            // Set up stdin
            if is_first {
                command.stdin(std::process::Stdio::inherit());
            } else {
                command.stdin(last_stdout.take().unwrap());
            }

            // Set up stdout
            if is_last {
                command.stdout(std::process::Stdio::inherit());
                command.stderr(std::process::Stdio::inherit());
            } else {
                command.stdout(std::process::Stdio::piped());
                command.stderr(std::process::Stdio::inherit());
            }

            // Set environment
            command.env_clear();
            for (k, v) in &self.env {
                command.env(k, v);
            }

            let mut child = command.spawn()?;

            if !is_last {
                last_stdout = child.stdout.take();
            }

            children.push(child);
        }

        // Wait for all children
        let mut last_status = ExitStatus::SUCCESS;
        for mut child in children {
            let status = child.wait()?;
            last_status = ExitStatus::from_process(status);
        }

        Ok(last_status)
    }

    fn find_in_path(&self, program: &str) -> Result<std::path::PathBuf, ExecError> {
        // If it contains a slash, treat as path
        if program.contains('/') {
            let path = std::path::PathBuf::from(program);
            if path.exists() {
                return Ok(path);
            }
            return Err(ExecError::CommandNotFound(program.to_string()));
        }

        // Search in PATH
        let path_var = self.env.get("PATH").cloned().unwrap_or_default();
        for dir in path_var.split(':') {
            let candidate = std::path::PathBuf::from(dir).join(program);
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        Err(ExecError::CommandNotFound(program.to_string()))
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}
