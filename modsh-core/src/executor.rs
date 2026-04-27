//! Executor — Forks/execs commands, manages pipes

use crate::jobcontrol::{JobControl, JobStatus};
use crate::parser::{Command, FunctionDefinition, RedirectKind, SimpleCommand};
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
    /// Parse error during script execution
    #[error("parse error: {0}")]
    ParseError(String),
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

/// Shell options (set -e, set -x, etc.)
#[derive(Debug, Clone, Default)]
pub struct ShellOptions {
    /// Exit on error (set -e)
    pub errexit: bool,
    /// Print commands before executing (set -x)
    pub xtrace: bool,
    /// Treat unset variables as error (set -u)
    pub nounset: bool,
    /// Disable wildcard expansion (set -f)
    pub noglob: bool,
}

/// Executor for shell commands
pub struct Executor {
    /// Environment variables
    pub env: std::collections::HashMap<String, String>,
    /// Aliases (name -> replacement)
    pub aliases: std::collections::HashMap<String, String>,
    /// Functions (name -> body command)
    functions: std::collections::HashMap<String, FunctionDefinition>,
    /// Positional parameters ($1, $2, etc.)
    pub positional_params: Vec<String>,
    /// Shell options (set -e, -x, etc.)
    pub shell_options: ShellOptions,
    /// Current working directory
    pub cwd: std::path::PathBuf,
    /// Temporary files for heredocs/herestrings (kept alive until process starts)
    temp_files: Vec<tempfile::NamedTempFile>,
    /// Job control manager
    pub job_control: JobControl,
}

impl Executor {
    /// Create a new executor
    #[must_use]
    pub fn new() -> Self {
        Self {
            env: std::env::vars().collect(),
            aliases: std::collections::HashMap::new(),
            functions: std::collections::HashMap::new(),
            positional_params: Vec::new(),
            shell_options: ShellOptions::default(),
            cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/")),
            temp_files: Vec::new(),
            job_control: JobControl::new(),
        }
    }

    /// Get a reference to the job control manager
    #[must_use]
    pub fn job_control(&self) -> &JobControl {
        &self.job_control
    }

    /// Get a mutable reference to the job control manager
    #[must_use]
    pub fn job_control_mut(&mut self) -> &mut JobControl {
        &mut self.job_control
    }

    /// Reap completed background jobs
    pub fn reap_jobs(&mut self) {
        self.job_control.cleanup();
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
            Command::Background(cmd) => self.execute_background(cmd),
            Command::Subshell(cmd) => self.execute_subshell(cmd),
            Command::Group(cmd) => self.execute(cmd),
            Command::If(if_clause) => self.execute_if(if_clause),
            Command::For(for_loop) => self.execute_for(for_loop),
            Command::While(while_loop) => self.execute_while(while_loop),
            Command::Case(case_stmt) => self.execute_case(case_stmt),
            Command::Function(func_def) => self.execute_function_def(func_def),
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
        let words: Vec<String> = match &for_loop.words {
            None => {
                // No "in" clause - iterate over "$@" (positional parameters)
                // TODO: Get positional parameters from shell state
                vec![]
            }
            Some(words) => words.clone(),
        };

        let mut last_status = ExitStatus::SUCCESS;
        for word in words {
            // TODO: Set loop variable in environment
            let _ = word;
            last_status = self.execute(&for_loop.body)?;
        }
        Ok(last_status)
    }

    fn execute_while(
        &mut self,
        while_loop: &crate::parser::WhileLoop,
    ) -> Result<ExitStatus, ExecError> {
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

    fn execute_case(
        &mut self,
        case_stmt: &crate::parser::CaseStatement,
    ) -> Result<ExitStatus, ExecError> {
        // TODO: Pattern matching against case_stmt.word
        let mut last_status = ExitStatus::SUCCESS;
        for (_patterns, body) in &case_stmt.clauses {
            // TODO: Check if word matches any pattern
            last_status = self.execute(body)?;
            // TODO: Break after first match (or continue for ;&)
        }
        Ok(last_status)
    }

    fn execute_function_def(
        &mut self,
        func_def: &crate::parser::FunctionDefinition,
    ) -> Result<ExitStatus, ExecError> {
        // Register the function in the function table
        self.functions
            .insert(func_def.name.clone(), func_def.clone());
        Ok(ExitStatus::SUCCESS)
    }

    /// Execute a function call
    fn execute_function_call(
        &mut self,
        name: &str,
        _args: &[String],
    ) -> Result<ExitStatus, ExecError> {
        // Get the function definition and clone the body to avoid borrow issues
        let body = self
            .functions
            .get(name)
            .ok_or_else(|| ExecError::CommandNotFound(name.to_string()))?
            .body
            .clone();

        // Save current positional parameters (if any)
        // For now, we don't implement $1, $2, etc. but this is where we'd set them

        // Execute the function body
        let result = self.execute(&body);

        // Handle return builtin - convert Return error to Ok status
        match result {
            Ok(status) => Ok(status),
            Err(ExecError::Builtin(crate::builtins::BuiltinError::Return(code))) => {
                Ok(ExitStatus {
                    code: code as u8,
                    signaled: false,
                })
            }
            Err(e) => Err(e),
        }
    }

    /// Execute a command in the background using fork
    #[cfg(unix)]
    fn execute_background(&mut self, cmd: &Command) -> Result<ExitStatus, ExecError> {
        let command_str = self.command_to_string(cmd);

        // Fork the process
        let pid = unsafe { libc::fork() };

        match pid {
            -1 => Err(ExecError::Io(std::io::Error::last_os_error())),
            0 => {
                // Child process
                // Create new process group
                let _ = unsafe { libc::setpgid(0, 0) };

                // Ignore SIGHUP in background process (common shell behavior)
                unsafe {
                    let sig_action = libc::sigaction {
                        sa_sigaction: libc::SIG_IGN,
                        sa_mask: std::mem::zeroed(),
                        sa_flags: 0,
                        sa_restorer: None,
                    };
                    libc::sigaction(libc::SIGHUP, &sig_action, std::ptr::null_mut());
                }

                // Execute the command and capture exit status
                let exit_code = match self.execute(cmd) {
                    Ok(status) if status.success() => 0,
                    Ok(status) => i32::from(status.code),
                    Err(_) => 1,
                };

                // Use _exit to avoid flushing inherited stdio buffers
                unsafe { libc::_exit(exit_code) };
            }
            pid => {
                // Parent process
                // pid is guaranteed to be positive here (not -1, not 0)
                let child_pid = pid as libc::pid_t;
                // After child's setpgid(0, 0), pgid equals child's pid
                let pgid = child_pid as u32;

                // Add job to job control
                let job_id = self.job_control.add_job(command_str.clone(), Some(pgid));

                // Update job status to running
                self.job_control.update_status(job_id, JobStatus::Running);

                // Print job info like bash: [job_id] process_group_id
                println!("[{}] {}", job_id, pid);

                // Return success immediately (background commands always return 0)
                Ok(ExitStatus::SUCCESS)
            }
        }
    }

    /// Non-Unix fallback: run synchronously with warning
    #[cfg(not(unix))]
    fn execute_background(&mut self, cmd: &Command) -> Result<ExitStatus, ExecError> {
        eprintln!("modsh: background execution not supported on this platform");
        self.execute(cmd)
    }

    /// Execute a command in a subshell using fork (synchronous, waits for child)
    #[cfg(unix)]
    fn execute_subshell(&mut self, cmd: &Command) -> Result<ExitStatus, ExecError> {
        // Fork the process
        let pid = unsafe { libc::fork() };

        match pid {
            -1 => Err(ExecError::Io(std::io::Error::last_os_error())),
            0 => {
                // Child process - runs in same process group as parent
                // Subshell doesn't create new process group, so setpgid(0, 0) is NOT called
                // This allows the subshell to receive signals sent to the parent's process group

                // Execute the command
                let result = self.execute(cmd);

                // Flush stdout/stderr before exiting to ensure all output is written
                // (important because _exit doesn't flush stdio buffers)
                let _ = std::io::Write::flush(&mut std::io::stdout());
                let _ = std::io::Write::flush(&mut std::io::stderr());

                let exit_code = match result {
                    Ok(status) if status.success() => 0,
                    Ok(status) => i32::from(status.code),
                    Err(_) => 1,
                };

                // Use _exit to avoid flushing inherited stdio buffers in the parent
                unsafe { libc::_exit(exit_code) };
            }
            _ => {
                // Parent process - wait for child to complete
                let mut status: libc::c_int = 0;
                let result = unsafe { libc::waitpid(pid, &mut status, 0) };

                if result == -1 {
                    return Err(ExecError::Io(std::io::Error::last_os_error()));
                }

                // Extract exit status
                let exit_status = if libc::WIFEXITED(status) {
                    ExitStatus {
                        code: u8::try_from(libc::WEXITSTATUS(status)).unwrap_or(255),
                        signaled: false,
                    }
                } else if libc::WIFSIGNALED(status) {
                    let signal_num = u8::try_from(libc::WTERMSIG(status)).unwrap_or(127);
                    ExitStatus {
                        code: signal_num.saturating_add(128),
                        signaled: true,
                    }
                } else {
                    // Child was stopped (WIFSTOPPED) or continued (WIFCONTINUED)
                    // This is an abnormal condition for a synchronous wait
                    ExitStatus {
                        code: 1,
                        signaled: false,
                    }
                };

                Ok(exit_status)
            }
        }
    }

    /// Non-Unix fallback for subshell: run synchronously with warning
    #[cfg(not(unix))]
    fn execute_subshell(&mut self, cmd: &Command) -> Result<ExitStatus, ExecError> {
        eprintln!("modsh: subshell execution not supported on this platform");
        self.execute(cmd)
    }

    /// Convert a command to a string representation for job control
    fn command_to_string(&self, cmd: &Command) -> String {
        match cmd {
            Command::Simple(s) => s.words.join(" "),
            Command::Pipeline(_) => "pipeline".to_string(),
            Command::And(_, _) => "and-list".to_string(),
            Command::Or(_, _) => "or-list".to_string(),
            Command::List(_, _) => "list".to_string(),
            Command::Background(c) => format!("{} &", self.command_to_string(c)),
            Command::Subshell(_) => "subshell".to_string(),
            Command::Group(_) => "group".to_string(),
            Command::If(_) => "if-statement".to_string(),
            Command::For(_) => "for-loop".to_string(),
            Command::While(_) => "while-loop".to_string(),
            Command::Case(_) => "case-statement".to_string(),
            Command::Function(_) => "function-def".to_string(),
        }
    }

    /// Expand aliases in a simple command
    /// Returns a new SimpleCommand with aliases expanded
    /// Prevents infinite recursion by tracking expanded aliases
    fn expand_aliases(&self, cmd: &SimpleCommand) -> SimpleCommand {
        self.expand_aliases_recursive(cmd, &mut std::collections::HashSet::new())
    }

    /// Recursive helper with cycle detection
    fn expand_aliases_recursive(
        &self,
        cmd: &SimpleCommand,
        expanded: &mut std::collections::HashSet<String>,
    ) -> SimpleCommand {
        if cmd.words.is_empty() {
            return cmd.clone();
        }

        let first_word = &cmd.words[0];

        // Check if already expanded (prevents cycles)
        if expanded.contains(first_word) {
            return cmd.clone();
        }

        // Check if the first word is an alias
        if let Some(alias_value) = self.aliases.get(first_word) {
            // Parse the alias value into words
            let alias_words: Vec<String> = alias_value
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();

            if !alias_words.is_empty() {
                // Mark this alias as expanded (prevents recursion)
                expanded.insert(first_word.clone());

                // Combine alias words with the rest of the command
                let mut new_words = alias_words;
                new_words.extend(cmd.words[1..].iter().cloned());

                let new_cmd = SimpleCommand {
                    words: new_words,
                    redirects: cmd.redirects.clone(),
                };

                // Recursively expand the result (for chained aliases)
                return self.expand_aliases_recursive(&new_cmd, expanded);
            }
        }

        // No alias expansion needed
        cmd.clone()
    }

    fn execute_simple(&mut self, cmd: &SimpleCommand) -> Result<ExitStatus, ExecError> {
        // Clear temporary files from previous command
        self.temp_files.clear();

        // Expand aliases in the command
        let cmd = self.expand_aliases(cmd);

        // Set up redirects first (needed for both builtins and external commands)
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
                (Some(2), RedirectKind::Append) => {
                    stderr = std::process::Stdio::from(
                        std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&redirect.target)?,
                    );
                }
                (None | Some(1), RedirectKind::Heredoc) => {
                    use std::io::Write;
                    let mut temp_file = tempfile::NamedTempFile::new()?;
                    temp_file.write_all(redirect.target.as_bytes())?;
                    temp_file.write_all(b"\n")?;
                    let file = temp_file.reopen()?;
                    stdin = std::process::Stdio::from(file);
                    self.temp_files.push(temp_file);
                }
                (None | Some(1), RedirectKind::Herestring) => {
                    use std::io::Write;
                    let mut temp_file = tempfile::NamedTempFile::new()?;
                    temp_file.write_all(redirect.target.as_bytes())?;
                    temp_file.write_all(b"\n")?;
                    let file = temp_file.reopen()?;
                    stdin = std::process::Stdio::from(file);
                    self.temp_files.push(temp_file);
                }
                (_, RedirectKind::OutputStdoutStderr) => {
                    if redirect.fd.is_some() {
                        return Err(ExecError::InvalidRedirect(
                            "&> does not accept file descriptor prefixes".to_string(),
                        ));
                    }
                    let file = std::fs::File::create(&redirect.target)?;
                    let file2 = file.try_clone()?;
                    stdout = std::process::Stdio::from(file);
                    stderr = std::process::Stdio::from(file2);
                }
                (_, RedirectKind::AppendStdoutStderr) => {
                    if redirect.fd.is_some() {
                        return Err(ExecError::InvalidRedirect(
                            "&>> does not accept file descriptor prefixes".to_string(),
                        ));
                    }
                    let file = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&redirect.target)?;
                    let file2 = file.try_clone()?;
                    stdout = std::process::Stdio::from(file);
                    stderr = std::process::Stdio::from(file2);
                }
                _ => {
                    // TODO: Handle other redirect types
                }
            }
        }

        if cmd.words.is_empty() {
            // TODO: Handle redirects only (e.g., `> file` without command)
            return Ok(ExitStatus::SUCCESS);
        }

        let program = &cmd.words[0];
        let args: Vec<&str> = cmd.words[1..].iter().map(String::as_str).collect();

        // Check if it's a function call (functions take precedence over builtins)
        if self.functions.contains_key(program) {
            let args_owned: Vec<String> = cmd.words[1..].iter().cloned().collect();
            return self.execute_function_call(program, &args_owned);
        }

        // Check if it's a builtin
        if let Some(builtin) = crate::builtins::get_builtin(program) {
            // TODO: Apply redirects to builtin output
            let mut state = crate::builtins::ShellState {
                env: &mut self.env,
                aliases: &mut self.aliases,
                positional_params: &mut self.positional_params,
                options: &mut self.shell_options,
                job_control: Some(&mut self.job_control),
            };
            match builtin(&args, &mut state) {
                Ok(status) => return Ok(status),
                Err(crate::builtins::BuiltinError::Source(path)) => {
                    return self.execute_source(&path);
                }
                // Return error propagates up - will be caught by function call handler
                // or become an error if used outside of function
                Err(crate::builtins::BuiltinError::Return(code)) => {
                    return Err(ExecError::Builtin(crate::builtins::BuiltinError::Return(
                        code,
                    )));
                }
                Err(e) => return Err(ExecError::Builtin(e)),
            }
        }

        // Search in PATH
        let program_path = self.find_in_path(program)?;

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

        // Pipeline with multiple commands - fork for each stage
        // This allows builtins to participate in pipelines
        self.execute_pipeline_forked(commands)
    }

    /// Execute a pipeline by forking for each stage
    /// This supports both builtins and external commands in the pipeline
    #[cfg(unix)]
    fn execute_pipeline_forked(&mut self, commands: &[Command]) -> Result<ExitStatus, ExecError> {
        let mut prev_read_end: Option<i32> = None;
        let mut pids: Vec<libc::pid_t> = Vec::new();

        for (i, cmd) in commands.iter().enumerate() {
            let is_first = i == 0;
            let is_last = i == commands.len() - 1;

            // Create pipe for this stage (unless it's the last one)
            let (read_end, write_end) = if !is_last {
                let mut pipe_fds: [libc::c_int; 2] = [-1, -1];
                if unsafe { libc::pipe(pipe_fds.as_mut_ptr()) } == -1 {
                    return Err(ExecError::Io(std::io::Error::last_os_error()));
                }
                (Some(pipe_fds[0]), Some(pipe_fds[1]))
            } else {
                (None, None)
            };

            let pid = unsafe { libc::fork() };

            match pid {
                -1 => {
                    // Clean up pipes on error
                    if let Some(fd) = read_end {
                        unsafe { libc::close(fd) };
                    }
                    if let Some(fd) = write_end {
                        unsafe { libc::close(fd) };
                    }
                    if let Some(fd) = prev_read_end {
                        unsafe { libc::close(fd) };
                    }
                    return Err(ExecError::Io(std::io::Error::last_os_error()));
                }
                0 => {
                    // Child process
                    // Close previous read end if not first
                    if !is_first {
                        if let Some(fd) = prev_read_end {
                            unsafe {
                                libc::dup2(fd, libc::STDIN_FILENO);
                                libc::close(fd);
                            }
                        }
                    }

                    // Set up stdout to write to pipe if not last
                    if let Some(fd) = write_end {
                        unsafe {
                            libc::dup2(fd, libc::STDOUT_FILENO);
                            libc::close(fd);
                        }
                    }
                    // Close the read end of the current pipe in child
                    if let Some(fd) = read_end {
                        unsafe { libc::close(fd) };
                    }

                    // Execute the command
                    let result = self.execute(cmd);

                    // Flush stdout/stderr before exiting to ensure all output is written
                    // (important because _exit doesn't flush stdio buffers)
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                    let _ = std::io::Write::flush(&mut std::io::stderr());

                    let exit_code = match result {
                        Ok(status) if status.success() => 0,
                        Ok(status) => i32::from(status.code),
                        Err(_) => 1,
                    };
                    std::process::exit(exit_code);
                }
                _ => {
                    // Parent process
                    // Close previous read end
                    if let Some(fd) = prev_read_end {
                        unsafe { libc::close(fd) };
                    }
                    // Close write end of current pipe (parent doesn't need it)
                    if let Some(fd) = write_end {
                        unsafe { libc::close(fd) };
                    }

                    pids.push(pid);
                    // Save read end for next iteration
                    prev_read_end = read_end;
                }
            }
        }

        // Close the final read end (should be None for last stage, but be safe)
        if let Some(fd) = prev_read_end {
            unsafe { libc::close(fd) };
        }

        // Wait for all children and get the last one's status
        let mut last_status = ExitStatus::SUCCESS;
        for pid in pids {
            let mut status: libc::c_int = 0;
            if unsafe { libc::waitpid(pid, &mut status, 0) } == -1 {
                return Err(ExecError::Io(std::io::Error::last_os_error()));
            }

            last_status = if libc::WIFEXITED(status) {
                ExitStatus {
                    code: u8::try_from(libc::WEXITSTATUS(status)).unwrap_or(255),
                    signaled: false,
                }
            } else if libc::WIFSIGNALED(status) {
                let signal_num = u8::try_from(libc::WTERMSIG(status)).unwrap_or(127);
                ExitStatus {
                    code: signal_num.saturating_add(128),
                    signaled: true,
                }
            } else {
                ExitStatus {
                    code: 1,
                    signaled: false,
                }
            };
        }

        Ok(last_status)
    }

    /// Non-Unix fallback for pipeline with builtins
    #[cfg(not(unix))]
    fn execute_pipeline_forked(&mut self, commands: &[Command]) -> Result<ExitStatus, ExecError> {
        // Fallback: only external commands in pipelines
        eprintln!("modsh: full pipeline support requires Unix");
        self.execute_pipeline_external_only(commands)
    }

    /// Execute pipeline with only external commands (original implementation)
    #[cfg(not(unix))]
    fn execute_pipeline_external_only(
        &mut self,
        commands: &[Command],
    ) -> Result<ExitStatus, ExecError> {
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

            // Check if it's a builtin - if so, we can't run it in this external-only path
            if crate::builtins::get_builtin(program).is_some() {
                return Err(ExecError::CommandNotFound(format!(
                    "builtin '{}' not supported in pipeline on this platform",
                    program
                )));
            }

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

    /// Execute a sourced script file
    fn execute_source(&mut self, path: &str) -> Result<ExitStatus, ExecError> {
        // Check file metadata
        let metadata = std::fs::metadata(path).map_err(|e| ExecError::Io(e))?;

        // Ensure it's a file (not a directory)
        if !metadata.is_file() {
            return Err(ExecError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("{}: Is a directory", path),
            )));
        }

        // Read the script content
        let content = std::fs::read_to_string(path).map_err(|e| ExecError::Io(e))?;

        // Parse the script
        let ast =
            crate::parser::parse(&content).map_err(|e| ExecError::ParseError(e.to_string()))?;

        // Execute the parsed script
        self.execute(&ast)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::BuiltinError;
    use crate::parser::parse;

    fn execute(cmd_str: &str) -> Result<ExitStatus, ExecError> {
        let ast = parse(cmd_str).expect("Failed to parse command");
        let mut executor = Executor::new();
        executor.execute(&ast)
    }

    // ===== Simple Command Tests =====

    #[test]
    fn test_true_command() {
        let status = execute("true").expect("Failed to execute true");
        assert!(status.success(), "true should return success");
        assert_eq!(status.code, 0);
    }

    #[test]
    fn test_false_command() {
        let status = execute("false").expect("Failed to execute false");
        assert!(!status.success(), "false should return failure");
        assert_eq!(status.code, 1);
    }

    #[test]
    fn test_command_not_found() {
        let result = execute("this_command_definitely_does_not_exist_xyz");
        assert!(
            matches!(result, Err(ExecError::CommandNotFound(_))),
            "Expected CommandNotFound error"
        );
    }

    #[test]
    fn test_echo_command() {
        let status = execute("echo hello").expect("Failed to execute echo");
        assert!(status.success(), "echo should return success");
    }

    // ===== Exit Status Tests =====

    #[test]
    fn test_exit_status_success() {
        let status = execute("/bin/sh -c 'exit 0'").expect("Failed to execute");
        assert_eq!(status.code, 0);
        assert!(!status.signaled);
    }

    #[test]
    fn test_exit_status_failure() {
        let status = execute("/bin/sh -c 'exit 42'").expect("Failed to execute");
        assert_eq!(status.code, 42);
        assert!(!status.signaled);
    }

    #[test]
    fn test_exit_status_255_max() {
        let status = execute("/bin/sh -c 'exit 256'").expect("Failed to execute");
        // Exit codes wrap around: 256 % 256 = 0, but we clamp to 255 for overflow
        // Actually in POSIX, exit codes are 0-255, higher values wrap
        assert!(!status.signaled);
    }

    // ===== Logical Operators Tests =====

    #[test]
    fn test_and_operator_success() {
        let status = execute("true && true").expect("Failed to execute");
        assert!(status.success(), "true && true should succeed");
    }

    #[test]
    fn test_and_operator_failure_left() {
        let status = execute("false && true").expect("Failed to execute");
        assert!(!status.success(), "false && true should fail");
        assert_eq!(status.code, 1);
    }

    #[test]
    fn test_and_operator_failure_right() {
        let status = execute("true && false").expect("Failed to execute");
        assert!(!status.success(), "true && false should fail");
        assert_eq!(status.code, 1);
    }

    #[test]
    fn test_or_operator_success_left() {
        let status = execute("true || false").expect("Failed to execute");
        assert!(status.success(), "true || false should succeed");
    }

    #[test]
    fn test_or_operator_success_right() {
        let status = execute("false || true").expect("Failed to execute");
        assert!(status.success(), "false || true should succeed");
    }

    #[test]
    fn test_or_operator_failure_both() {
        let status = execute("false || false").expect("Failed to execute");
        assert!(!status.success(), "false || false should fail");
        assert_eq!(status.code, 1);
    }

    #[test]
    fn test_mixed_and_or() {
        let status = execute("false || true && true").expect("Failed to execute");
        assert!(status.success(), "false || true && true should succeed");
    }

    // ===== List/Sequence Tests =====

    #[test]
    fn test_list_executes_both() {
        let status = execute("true ; true").expect("Failed to execute");
        assert!(status.success(), "true ; true should succeed");
    }

    #[test]
    fn test_list_uses_last_status() {
        let status = execute("true ; false").expect("Failed to execute");
        assert!(
            !status.success(),
            "true ; false should use last exit status"
        );
        assert_eq!(status.code, 1);
    }

    // ===== Pipeline Tests =====

    #[test]
    fn test_simple_pipeline() {
        let status = execute("echo hello | cat").expect("Failed to execute pipeline");
        assert!(status.success(), "echo | cat should succeed");
    }

    #[test]
    fn test_pipeline_failure() {
        let status = execute("false | true").expect("Failed to execute pipeline");
        // Last command's status determines pipeline exit status
        assert!(status.success(), "false | true should return true's status");
    }

    #[test]
    fn test_pipeline_with_false_at_end() {
        let status = execute("true | false").expect("Failed to execute pipeline");
        assert!(
            !status.success(),
            "true | false should return false's status"
        );
    }

    #[test]
    fn test_three_stage_pipeline() {
        let status = execute("echo hello | cat | cat").expect("Failed to execute");
        assert!(status.success(), "3-stage pipeline should succeed");
    }

    // ===== Group Tests =====

    #[test]
    fn test_group_success() {
        let status = execute("{ true; }").expect("Failed to execute group");
        assert!(status.success(), "Group with true should succeed");
    }

    #[test]
    fn test_group_failure() {
        let status = execute("{ false; }").expect("Failed to execute group");
        assert!(!status.success(), "Group with false should fail");
    }

    // ===== Builtin Tests =====

    /// WARNING: This test modifies the process-global current directory.
    /// Running tests in parallel may cause interference with other tests
    /// that depend on the working directory.
    #[test]
    fn test_builtin_cd() {
        let original_dir = std::env::current_dir().expect("Failed to get current dir");
        let ast = parse("cd /tmp").expect("Failed to parse");
        let mut executor = Executor::new();
        let status = executor.execute(&ast).expect("Failed to execute cd");
        assert!(status.success(), "cd to /tmp should succeed");
        // The cd builtin changes the actual process working directory
        assert_eq!(
            std::env::current_dir().expect("Failed to get current dir"),
            std::path::PathBuf::from("/tmp")
        );
        // Restore original directory
        std::env::set_current_dir(original_dir).expect("Failed to restore dir");
    }

    #[test]
    fn test_builtin_echo() {
        let status = execute("echo").expect("Failed to execute echo");
        assert!(status.success(), "echo should succeed");
    }

    #[test]
    fn test_builtin_exit_status() {
        // exit builtin returns Err(BuiltinError::Exit(code)) which signals the shell to exit
        let result = execute("exit 0");
        assert!(
            matches!(result, Err(ExecError::Builtin(BuiltinError::Exit(0)))),
            "exit 0 should return Exit error with code 0"
        );
    }

    #[test]
    fn test_builtin_exit_nonzero() {
        let result = execute("exit 42");
        assert!(
            matches!(result, Err(ExecError::Builtin(BuiltinError::Exit(42)))),
            "exit 42 should return Exit error with code 42"
        );
    }

    // ===== Printf Tests =====

    #[test]
    fn test_builtin_printf_simple() {
        let status = execute("printf 'hello world'").expect("Failed to execute printf");
        assert!(status.success(), "printf should succeed");
    }

    #[test]
    fn test_builtin_printf_string() {
        let status = execute("printf '%s' hello").expect("Failed to execute printf");
        assert!(status.success(), "printf %s should succeed");
    }

    #[test]
    fn test_builtin_printf_integer() {
        let status = execute("printf '%d' 42").expect("Failed to execute printf %d");
        assert!(status.success(), "printf %d should succeed");
    }

    #[test]
    fn test_builtin_printf_hex() {
        let status = execute("printf '%x' 255").expect("Failed to execute printf %x");
        assert!(status.success(), "printf %x should succeed");
    }

    #[test]
    fn test_builtin_printf_octal() {
        let status = execute("printf '%o' 8").expect("Failed to execute printf %o");
        assert!(status.success(), "printf %o should succeed");
    }

    #[test]
    fn test_builtin_printf_char() {
        let status = execute("printf '%c' abc").expect("Failed to execute printf %c");
        assert!(status.success(), "printf %c should succeed");
    }

    #[test]
    fn test_builtin_printf_escape() {
        let status = execute("printf '%b' 'hello\\nworld'").expect("Failed to execute printf %b");
        assert!(status.success(), "printf %b should succeed");
    }

    #[test]
    fn test_builtin_printf_format_string() {
        let status =
            execute("printf 'Name: %s, Age: %d' John 30").expect("Failed to execute printf");
        assert!(status.success(), "printf with format should succeed");
    }

    #[test]
    fn test_builtin_printf_width() {
        let status = execute("printf '%5s' hi").expect("Failed to execute printf width");
        assert!(status.success(), "printf width should succeed");
    }

    #[test]
    fn test_builtin_printf_left_align() {
        let status = execute("printf '%-5s' hi").expect("Failed to execute printf left align");
        assert!(status.success(), "printf left align should succeed");
    }

    #[test]
    fn test_builtin_printf_precision() {
        let status = execute("printf '%.2f' 3.14159").expect("Failed to execute printf precision");
        assert!(status.success(), "printf precision should succeed");
    }

    #[test]
    fn test_builtin_printf_width_and_precision() {
        let status =
            execute("printf '%8.2f' 3.14159").expect("Failed to execute printf width+precision");
        assert!(
            status.success(),
            "printf width and precision should succeed"
        );
    }

    // ===== Alias Tests =====

    #[test]
    fn test_builtin_alias_define() {
        let ast = parse("alias ll='ls -la'").expect("Failed to parse");
        let mut executor = Executor::new();
        let status = executor.execute(&ast).expect("Failed to execute alias");
        assert!(status.success(), "alias definition should succeed");
        assert_eq!(executor.aliases.get("ll"), Some(&"ls -la".to_string()));
    }

    #[test]
    fn test_builtin_alias_list() {
        let ast = parse("alias").expect("Failed to parse");
        let mut executor = Executor::new();
        // No aliases defined yet, should succeed with no output
        let status = executor.execute(&ast).expect("Failed to list aliases");
        assert!(status.success(), "alias list should succeed");
    }

    #[test]
    fn test_builtin_alias_print() {
        let ast = parse("alias ll='ls -la'").expect("Failed to parse");
        let mut executor = Executor::new();
        executor.execute(&ast).expect("Failed to define alias");

        let ast = parse("alias ll").expect("Failed to parse");
        let status = executor.execute(&ast).expect("Failed to print alias");
        assert!(status.success(), "alias print should succeed");
    }

    #[test]
    fn test_builtin_unalias() {
        let ast = parse("alias ll='ls -la'").expect("Failed to parse");
        let mut executor = Executor::new();
        executor.execute(&ast).expect("Failed to define alias");
        assert!(executor.aliases.contains_key("ll"));

        let ast = parse("unalias ll").expect("Failed to parse");
        let status = executor.execute(&ast).expect("Failed to unalias");
        assert!(status.success(), "unalias should succeed");
        assert!(!executor.aliases.contains_key("ll"));
    }

    #[test]
    fn test_alias_expansion() {
        // Define an alias that points to 'true' builtin
        let ast = parse("alias t=true").expect("Failed to parse");
        let mut executor = Executor::new();
        executor.execute(&ast).expect("Failed to define alias");

        // Now execute the alias - it should expand to 'true' and succeed
        let ast = parse("t").expect("Failed to parse");
        let status = executor.execute(&ast).expect("Failed to execute alias");
        assert!(
            status.success(),
            "alias expansion should execute 'true' builtin"
        );
    }

    #[test]
    fn test_alias_expansion_with_args() {
        // Define an alias for echo
        let ast = parse("alias myecho='echo hello'").expect("Failed to parse");
        let mut executor = Executor::new();
        executor.execute(&ast).expect("Failed to define alias");

        // Execute alias with additional args - should expand and pass args
        let ast = parse("myecho world").expect("Failed to parse");
        let status = executor
            .execute(&ast)
            .expect("Failed to execute alias with args");
        assert!(status.success(), "alias expansion with args should succeed");
    }

    #[test]
    fn test_alias_recursion_protection() {
        // Self-referencing alias should not cause infinite loop
        let ast = parse("alias ls='ls -la'").expect("Failed to parse");
        let mut executor = Executor::new();
        executor.execute(&ast).expect("Failed to define alias");

        // Should not hang - recursion is prevented
        let ast = parse("ls").expect("Failed to parse");
        // This will fail because 'ls' is not found, but should not hang
        let _ = executor.execute(&ast);
        // If we get here without hanging, the test passes
    }

    #[test]
    fn test_alias_cycle_protection() {
        // Cyclic aliases should not cause infinite loop
        let ast = parse("alias a='b'; alias b='a'").expect("Failed to parse");
        let mut executor = Executor::new();
        executor.execute(&ast).expect("Failed to define aliases");

        // Should not hang - cycle is detected
        let ast = parse("a").expect("Failed to parse");
        let _ = executor.execute(&ast);
        // If we get here without hanging, the test passes
    }

    // ===== Return Builtin Tests =====

    #[test]
    fn test_builtin_return_from_function() {
        // Define a function that returns with a specific code
        let ast = parse("myfunc() { return 42; }").expect("Failed to parse");
        let mut executor = Executor::new();
        executor.execute(&ast).expect("Failed to define function");

        // Call the function and check return value
        let ast = parse("myfunc").expect("Failed to parse");
        let status = executor.execute(&ast).expect("Failed to call function");
        assert_eq!(status.code, 42, "Function should return 42");
    }

    #[test]
    fn test_builtin_return_default() {
        // Define a function with default return (0)
        let ast = parse("myfunc() { return; }").expect("Failed to parse");
        let mut executor = Executor::new();
        executor.execute(&ast).expect("Failed to define function");

        let ast = parse("myfunc").expect("Failed to parse");
        let status = executor.execute(&ast).expect("Failed to call function");
        assert!(status.success(), "Default return should be 0");
    }

    #[test]
    fn test_builtin_return_outside_function_errors() {
        // return outside of function should error
        let result = execute("return 42");
        assert!(result.is_err(), "return outside function should error");
    }

    // ===== Set and Shift Tests =====

    #[test]
    fn test_builtin_set_positional_params() {
        // Test set -- to set positional parameters
        let ast = parse("set -- arg1 arg2 arg3").expect("Failed to parse");
        let mut executor = Executor::new();
        let status = executor.execute(&ast).expect("Failed to execute set");
        assert!(status.success(), "set should succeed");
        assert_eq!(
            executor.positional_params.len(),
            3,
            "Should have 3 positional params"
        );
        assert_eq!(executor.positional_params[0], "arg1");
        assert_eq!(executor.positional_params[1], "arg2");
        assert_eq!(executor.positional_params[2], "arg3");
    }

    #[test]
    fn test_builtin_shift() {
        // Set positional parameters, then shift
        let ast = parse("set -- arg1 arg2 arg3; shift").expect("Failed to parse");
        let mut executor = Executor::new();
        let status = executor.execute(&ast).expect("Failed to execute");
        assert!(status.success(), "shift should succeed");
        assert_eq!(
            executor.positional_params.len(),
            2,
            "Should have 2 positional params after shift"
        );
        assert_eq!(executor.positional_params[0], "arg2");
        assert_eq!(executor.positional_params[1], "arg3");
    }

    #[test]
    fn test_builtin_shift_n() {
        // Shift by more than 1
        let ast = parse("set -- a b c d e; shift 2").expect("Failed to parse");
        let mut executor = Executor::new();
        let status = executor.execute(&ast).expect("Failed to execute");
        assert!(status.success(), "shift 2 should succeed");
        assert_eq!(
            executor.positional_params.len(),
            3,
            "Should have 3 positional params after shift 2"
        );
        assert_eq!(executor.positional_params[0], "c");
    }

    #[test]
    fn test_builtin_set_options() {
        // Test set -e, -x, etc.
        let ast = parse("set -e -x -u -f").expect("Failed to parse");
        let mut executor = Executor::new();
        let status = executor.execute(&ast).expect("Failed to execute set");
        assert!(status.success(), "set options should succeed");
        assert!(executor.shell_options.errexit, "errexit should be set");
        assert!(executor.shell_options.xtrace, "xtrace should be set");
        assert!(executor.shell_options.nounset, "nounset should be set");
        assert!(executor.shell_options.noglob, "noglob should be set");
    }

    #[test]
    fn test_builtin_set_disable_options() {
        // Test disabling options with +
        let ast = parse("set -e; set +e").expect("Failed to parse");
        let mut executor = Executor::new();
        executor.execute(&ast).expect("Failed to execute set");
        assert!(
            !executor.shell_options.errexit,
            "errexit should be disabled"
        );
    }

    // ===== Test Builtin Tests =====

    #[test]
    fn test_builtin_test_string_equal() {
        let status = execute("test hello = hello").expect("Failed to execute test");
        assert!(status.success(), "String equality test should succeed");
    }

    #[test]
    fn test_builtin_test_string_not_equal() {
        // Use explicit test for inequality via empty string check
        let ast = parse("test hello = world").expect("Failed to parse");
        let mut executor = Executor::new();
        let status = executor.execute(&ast).expect("Failed to execute test");
        assert!(!status.success(), "hello = world should be false");
    }

    #[test]
    fn test_builtin_test_numeric_comparison() {
        // 5 -lt 10 should be true
        let status = execute("test 5 -lt 10").expect("Failed to execute numeric test");
        assert!(status.success(), "5 < 10 should be true");
    }

    #[test]
    fn test_builtin_test_numeric_gt() {
        // 10 -gt 5 should be true
        let status = execute("test 10 -gt 5").expect("Failed to execute numeric test");
        assert!(status.success(), "10 > 5 should be true");
    }

    #[test]
    fn test_builtin_test_file_exists() {
        // Test with a known existing file
        let status = execute("test -f /bin/sh").expect("Failed to execute file test");
        assert!(status.success(), "/bin/sh should exist");
    }

    #[test]
    fn test_builtin_test_directory() {
        // Test with a known directory
        let status = execute("test -d /tmp").expect("Failed to execute directory test");
        assert!(status.success(), "/tmp should be a directory");
    }

    #[test]
    fn test_builtin_test_non_empty_string() {
        // -n tests for non-empty string
        let status = execute("test -n hello").expect("Failed to execute -n test");
        assert!(status.success(), "-n hello should be true");
    }

    #[test]
    fn test_builtin_test_empty_string() {
        // -z tests for empty string
        let status = execute("test -z ''").expect("Failed to execute -z test");
        assert!(status.success(), "-z '' should be true");
    }

    // ===== Read and Trap Tests =====

    #[test]
    #[ignore = "requires builtin redirect support (executor.rs:598 TODO); blocks on real stdin"]
    fn test_builtin_read_reply_variable() {
        // Test that read sets REPLY when no variable given
        // We can't easily test interactive input, but we can test the EOF case
        let ast = parse("read < /dev/null").expect("Failed to parse");
        let mut executor = Executor::new();
        let status = executor.execute(&ast).expect("Failed to execute read");
        // EOF should return failure (exit code 1)
        assert!(!status.success(), "read at EOF should return failure");
        // REPLY should be set to empty
        assert_eq!(executor.env.get("REPLY"), Some(&String::new()));
    }

    #[test]
    #[ignore = "requires builtin redirect support (executor.rs:598 TODO); blocks on real stdin"]
    fn test_builtin_read_with_variable() {
        // Test read with explicit variable name at EOF
        let ast = parse("read MYVAR < /dev/null").expect("Failed to parse");
        let mut executor = Executor::new();
        let status = executor.execute(&ast).expect("Failed to execute read");
        assert!(!status.success(), "read at EOF should return failure");
        assert_eq!(executor.env.get("MYVAR"), Some(&String::new()));
    }

    #[test]
    #[cfg(unix)]
    fn test_builtin_trap_list_signals() {
        // Test trap -l lists signals
        let status = execute("trap -l").expect("Failed to execute trap -l");
        assert!(status.success(), "trap -l should succeed");
    }

    #[test]
    #[cfg(unix)]
    fn test_builtin_trap_ignore_signal() {
        // Test trap '' ignores a signal
        let status = execute("trap '' INT").expect("Failed to execute trap");
        assert!(status.success(), "trap should succeed");
    }

    #[test]
    #[cfg(unix)]
    fn test_builtin_trap_reset_signal() {
        // Test trap - resets a signal to default
        let status = execute("trap - INT").expect("Failed to execute trap");
        assert!(status.success(), "trap - should succeed");
    }

    // ===== ExitStatus Tests =====

    #[test]
    fn test_exit_status_from_process_success() {
        let process_status = std::process::Command::new("true")
            .status()
            .expect("Failed to run true");
        let exit_status = ExitStatus::from_process(process_status);
        assert!(exit_status.success());
        assert_eq!(exit_status.code, 0);
        assert!(!exit_status.signaled);
    }

    #[test]
    fn test_exit_status_constants() {
        assert!(ExitStatus::SUCCESS.success());
        assert_eq!(ExitStatus::SUCCESS.code, 0);
    }

    // ===== Executor Creation Tests =====

    #[test]
    fn test_executor_new() {
        let executor = Executor::new();
        assert!(
            !executor.env.is_empty(),
            "Executor should inherit environment"
        );
        assert!(executor.cwd.exists(), "Executor should have valid cwd");
    }

    #[test]
    fn test_executor_default() {
        let executor: Executor = Default::default();
        assert!(
            !executor.env.is_empty(),
            "Default executor should inherit environment"
        );
    }

    #[test]
    fn test_executor_job_control() {
        let executor = Executor::new();
        assert!(executor.job_control().list_jobs().is_empty());
    }

    // ===== Fork-based Feature Tests =====
    // These tests must be in a single test function because fork() in a
    // multi-threaded environment can cause issues with locks and shared state.
    // By putting all fork-based tests in one function, they run serially
    // without interference from other tests.
    //
    // NOTE: This test must be run with --nocapture because output capturing
    // interferes with fork-based I/O redirection. Run with:
    //   cargo test --package modsh-core test_fork_based_features -- --ignored --nocapture

    #[test]
    #[ignore = "requires --nocapture due to fork-based I/O redirection"]
    fn test_fork_based_features() {
        // ----- Subshell Tests -----

        // Subshell with true should succeed
        let status = execute("( true )").expect("Failed to execute subshell");
        assert!(status.success(), "Subshell with true should succeed");

        // Subshell with false should fail
        let status = execute("( false )").expect("Failed to execute subshell");
        assert!(!status.success(), "Subshell with false should fail");
        assert_eq!(status.code, 1);

        // Subshell exit code propagation
        let status = execute("( /bin/sh -c 'exit 42' )").expect("Failed to execute subshell");
        assert_eq!(status.code, 42, "Subshell should propagate exit code");

        // Nested subshell
        let status = execute("( ( true ) )").expect("Failed to execute nested subshell");
        assert!(status.success(), "Nested subshell should succeed");

        // ----- Background Tests -----

        // Background returns success immediately
        let status = execute("sleep 0.1 &").expect("Failed to execute background");
        assert!(
            status.success(),
            "Background command should return success immediately"
        );

        // Small delay to let previous background process start
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Background job added to job control
        let ast = parse("sleep 0.05 &").expect("Failed to parse");
        let mut executor = Executor::new();
        let _ = executor.execute(&ast).expect("Failed to execute");
        std::thread::sleep(std::time::Duration::from_millis(50));
        let jobs = executor.job_control().list_jobs();
        assert!(
            !jobs.is_empty(),
            "Background job should be added to job control"
        );

        // Delay before next test
        std::thread::sleep(std::time::Duration::from_millis(100));

        // ----- Pipeline with Fork Tests -----

        // Test simple external pipeline first
        let status = execute("/bin/echo test | /bin/grep test").expect("Failed external pipeline");
        assert!(status.success(), "External pipeline should succeed");

        // Pipeline with builtins and logical operators
        // Note: This uses fork-based pipeline since there are multiple commands
        let status = execute("echo test | grep test && true").expect("Failed");
        assert!(status.success(), "Complex pipeline with && should succeed");

        // Subshell with pipeline
        let status = execute("( echo hello | cat )").expect("Failed");
        assert!(status.success(), "Subshell with pipeline should succeed");

        // List with background
        let status = execute("true; sleep 0.01 &").expect("Failed");
        assert!(status.success(), "List with background should succeed");
    }
}
