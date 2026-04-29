//! Builtins — Shell builtin commands

use std::collections::HashMap;
use thiserror::Error;

/// Errors from builtin commands
#[derive(Error, Debug)]
pub enum BuiltinError {
    /// Generic error with message
    #[error("{0}")]
    Generic(String),
    /// Exit with status code
    #[error("exit {0}")]
    Exit(i32),
    /// Return from function with status code
    #[error("return {0}")]
    Return(i32),
    /// Source a file (contains file path)
    #[error("source {0}")]
    Source(String),
}

/// Result type for builtins
pub type BuiltinResult = Result<super::executor::ExitStatus, BuiltinError>;

/// Shell state accessible to builtins
pub struct ShellState<'a> {
    /// Environment variables
    pub env: &'a mut HashMap<String, String>,
    /// Aliases (name -> replacement)
    pub aliases: &'a mut HashMap<String, String>,
    /// Positional parameters ($1, $2, etc.)
    pub positional_params: &'a mut Vec<String>,
    /// Shell options (set -e, -x, etc.)
    pub options: &'a mut super::executor::ShellOptions,
    /// Job control manager (for fg, bg, jobs builtins)
    pub job_control: Option<&'a mut super::jobcontrol::JobControl>,
}

/// Builtin function type
pub type BuiltinFn = fn(&[&str], &mut ShellState<'_>) -> BuiltinResult;

/// Get a builtin by name
pub fn get_builtin(name: &str) -> Option<BuiltinFn> {
    match name {
        "cd" => Some(builtin_cd),
        "pwd" => Some(builtin_pwd),
        "echo" => Some(builtin_echo),
        "printf" => Some(builtin_printf),
        "export" => Some(builtin_export),
        "unset" => Some(builtin_unset),
        "env" => Some(builtin_env),
        "exit" => Some(builtin_exit),
        "return" => Some(builtin_return),
        "true" => Some(builtin_true),
        "false" => Some(builtin_false),
        "alias" => Some(builtin_alias),
        "unalias" => Some(builtin_unalias),
        "." | "source" => Some(builtin_source),
        "set" => Some(builtin_set),
        "shift" => Some(builtin_shift),
        "test" | "[" => Some(builtin_test),
        "read" => Some(builtin_read),
        "trap" => Some(builtin_trap),
        "jobs" => Some(builtin_jobs),
        "fg" => Some(builtin_fg),
        "bg" => Some(builtin_bg),
        _ => None,
    }
}

/// Change directory builtin
fn builtin_cd(args: &[&str], state: &mut ShellState<'_>) -> BuiltinResult {
    let target = if args.is_empty() {
        // No args: go to HOME
        state
            .env
            .get("HOME")
            .cloned()
            .unwrap_or_else(|| "/".to_string())
    } else {
        args[0].to_string()
    };

    let path = std::path::PathBuf::from(&target);
    match std::env::set_current_dir(&path) {
        Ok(()) => {
            // Update PWD and OLDPWD environment variables
            if let Ok(cwd) = std::env::current_dir() {
                let old_pwd = state.env.get("PWD").cloned();
                if let Some(old) = old_pwd {
                    state.env.insert("OLDPWD".to_string(), old);
                }
                state
                    .env
                    .insert("PWD".to_string(), cwd.to_string_lossy().to_string());
            }
            Ok(super::executor::ExitStatus::SUCCESS)
        }
        Err(e) => Err(BuiltinError::Generic(format!("cd: {target}: {e}"))),
    }
}

/// Print working directory builtin
#[allow(clippy::unnecessary_wraps)]
fn builtin_pwd(_args: &[&str], _state: &mut ShellState<'_>) -> BuiltinResult {
    let cwd = std::env::current_dir().map_err(|e| BuiltinError::Generic(e.to_string()))?;
    println!("{}", cwd.display());
    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Echo builtin
#[allow(clippy::unnecessary_wraps)]
fn builtin_echo(args: &[&str], _state: &mut ShellState<'_>) -> BuiltinResult {
    let mut newline = true;
    let mut start = 0;

    // Parse options
    for (i, arg) in args.iter().enumerate() {
        if *arg == "-n" {
            newline = false;
            start = i + 1;
        } else {
            break;
        }
    }

    let output = args[start..].join(" ");
    if newline {
        println!("{output}");
    } else {
        print!("{output}");
    }

    // Flush stdout to ensure output is written when stdout is a pipe
    // (Rust stdout uses block buffering when connected to a pipe)
    let _ = std::io::Write::flush(&mut std::io::stdout());

    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Printf builtin - formatted output
/// Supports POSIX format specifiers with width/precision: %s, %d, %i, %o, %u, %x, %X, %f, %c, %b (escape), %%
#[allow(clippy::too_many_lines)]
fn builtin_printf(args: &[&str], _state: &mut ShellState<'_>) -> BuiltinResult {
    if args.is_empty() {
        return Err(BuiltinError::Generic(
            "printf: missing format string".to_string(),
        ));
    }

    let format = args[0];
    let mut arg_idx = 1;
    let mut output = String::new();
    let mut chars = format.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '%' {
            // Parse format specifier: [flags][width][.precision]type
            let mut width: Option<usize> = None;
            let mut precision: Option<usize> = None;
            let mut left_align = false;

            // Parse flags
            while let Some(&c) = chars.peek() {
                if c == '-' {
                    left_align = true;
                    chars.next();
                } else {
                    break;
                }
            }

            // Parse width
            let mut width_str = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() {
                    width_str.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            if !width_str.is_empty() {
                width = width_str.parse().ok();
            }

            // Parse precision
            if let Some(&'.') = chars.peek() {
                chars.next(); // consume '.'
                let mut prec_str = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() {
                        prec_str.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !prec_str.is_empty() {
                    precision = prec_str.parse().ok();
                }
            }

            // Get format character
            let format_char = chars.next().unwrap_or('\0');

            match format_char {
                '%' => {
                    output.push('%');
                }
                's' => {
                    // String argument
                    let arg = args.get(arg_idx).unwrap_or(&"");
                    let s = match precision {
                        Some(p) if p < arg.len() => &arg[..p],
                        _ => arg,
                    };
                    output.push_str(&apply_width(s, width, left_align));
                    arg_idx += 1;
                }
                'd' | 'i' => {
                    // Signed decimal integer
                    let arg = args.get(arg_idx).unwrap_or(&"0");
                    let val = arg.parse::<i64>().unwrap_or(0);
                    let s = val.to_string();
                    output.push_str(&apply_width(&s, width, left_align));
                    arg_idx += 1;
                }
                'u' => {
                    // Unsigned decimal integer
                    let arg = args.get(arg_idx).unwrap_or(&"0");
                    let val = arg.parse::<u64>().unwrap_or(0);
                    let s = val.to_string();
                    output.push_str(&apply_width(&s, width, left_align));
                    arg_idx += 1;
                }
                'o' => {
                    // Unsigned octal
                    let arg = args.get(arg_idx).unwrap_or(&"0");
                    let val = arg.parse::<u64>().unwrap_or(0);
                    let s = format!("{val:o}");
                    output.push_str(&apply_width(&s, width, left_align));
                    arg_idx += 1;
                }
                'x' => {
                    // Unsigned hex lowercase
                    let arg = args.get(arg_idx).unwrap_or(&"0");
                    let val = arg.parse::<u64>().unwrap_or(0);
                    let s = format!("{val:x}");
                    output.push_str(&apply_width(&s, width, left_align));
                    arg_idx += 1;
                }
                'X' => {
                    // Unsigned hex uppercase
                    let arg = args.get(arg_idx).unwrap_or(&"0");
                    let val = arg.parse::<u64>().unwrap_or(0);
                    let s = format!("{val:X}");
                    output.push_str(&apply_width(&s, width, left_align));
                    arg_idx += 1;
                }
                'f' => {
                    // Floating point
                    let arg = args.get(arg_idx).unwrap_or(&"0");
                    let val = arg.parse::<f64>().unwrap_or(0.0);
                    let prec = precision.unwrap_or(6);
                    let s = format!("{val:.prec$}");
                    output.push_str(&apply_width(&s, width, left_align));
                    arg_idx += 1;
                }
                'c' => {
                    // First character of argument
                    let arg = args.get(arg_idx).unwrap_or(&"");
                    let c = arg.chars().next().unwrap_or('\0');
                    let s = c.to_string();
                    output.push_str(&apply_width(&s, width, left_align));
                    arg_idx += 1;
                }
                'b' => {
                    // String with backslash escapes
                    let arg = args.get(arg_idx).unwrap_or(&"");
                    let expanded = expand_escapes(arg);
                    let s = match precision {
                        Some(p) if p < expanded.len() => &expanded[..p],
                        _ => &expanded,
                    };
                    output.push_str(&apply_width(s, width, left_align));
                    arg_idx += 1;
                }
                '\n' => {
                    // Literal newline in format
                    output.push('\n');
                }
                '\0' => {
                    // Trailing %
                    output.push('%');
                }
                ch => {
                    // Unknown format specifier - just output it
                    output.push('%');
                    output.push(ch);
                }
            }
        } else if ch == '\\' {
            // Handle escape sequences in format string
            match chars.next() {
                Some('n') => output.push('\n'),
                Some('t') => output.push('\t'),
                Some('r') => output.push('\r'),
                Some('\\') | None => output.push('\\'),
                Some('a') => output.push('\x07'), // BEL
                Some('b') => output.push('\x08'), // BS
                Some('e') => output.push('\x1b'), // ESC
                Some('f') => output.push('\x0c'), // FF
                Some('v') => output.push('\x0b'), // VT
                Some(ch) => {
                    output.push('\\');
                    output.push(ch);
                }
            }
        } else {
            output.push(ch);
        }
    }

    print!("{output}");
    let _ = std::io::Write::flush(&mut std::io::stdout());

    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Apply width padding to a string
fn apply_width(s: &str, width: Option<usize>, left_align: bool) -> String {
    match width {
        Some(w) if s.len() < w => {
            let padding = " ".repeat(w - s.len());
            if left_align {
                format!("{s}{padding}")
            } else {
                format!("{padding}{s}")
            }
        }
        _ => s.to_string(),
    }
}

/// Expand backslash escape sequences in a string
fn expand_escapes(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('\\') | None => result.push('\\'),
                Some('a') => result.push('\x07'),
                Some('b') => result.push('\x08'),
                Some('e') => result.push('\x1b'),
                Some('f') => result.push('\x0c'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('v') => result.push('\x0b'),
                Some('0') => {
                    // Octal escape \0NNN (POSIX: 1-3 digits, max \0377 = 255)
                    let mut octal = String::new();
                    for _ in 0..3 {
                        if let Some(&c) = chars.peek() {
                            if c.is_ascii_digit() && c <= '7' {
                                octal.push(c);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                    }
                    if let Ok(val) = u32::from_str_radix(&octal, 8) {
                        // POSIX specifies octal values must be 0-255 (\0 to \0377)
                        if val <= 255 {
                            if let Some(c) = char::from_u32(val) {
                                result.push(c);
                            }
                        }
                        // If val > 255, POSIX behavior is undefined; we skip the character
                    }
                }
                Some(ch) => {
                    result.push('\\');
                    result.push(ch);
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Export builtin
#[allow(clippy::unnecessary_wraps)]
fn builtin_export(args: &[&str], state: &mut ShellState<'_>) -> BuiltinResult {
    if args.is_empty() {
        // Print all exported variables
        for (k, v) in state.env.iter() {
            println!("export {}={}", k, shlex::quote(v));
        }
        return Ok(super::executor::ExitStatus::SUCCESS);
    }

    for arg in args {
        if let Some((name, value)) = arg.split_once('=') {
            // export VAR=value
            state.env.insert(name.to_string(), value.to_string());
            std::env::set_var(name, value);
        } else {
            // export VAR (mark as exported, for now just ensure it exists)
            if let Some(value) = state.env.get(*arg).cloned() {
                std::env::set_var(arg, value);
            }
        }
    }

    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Unset builtin
#[allow(clippy::unnecessary_wraps)]
fn builtin_unset(args: &[&str], state: &mut ShellState<'_>) -> BuiltinResult {
    for arg in args {
        state.env.remove(*arg);
        std::env::remove_var(arg);
    }
    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Env builtin - print environment
#[allow(clippy::unnecessary_wraps)]
fn builtin_env(_args: &[&str], _state: &mut ShellState<'_>) -> BuiltinResult {
    for (k, v) in std::env::vars() {
        println!("{k}={v}");
    }
    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Exit builtin
fn builtin_exit(args: &[&str], _state: &mut ShellState<'_>) -> BuiltinResult {
    let code = if args.is_empty() {
        0
    } else {
        args[0].parse::<i32>().unwrap_or(0)
    };
    Err(BuiltinError::Exit(code))
}

/// Return builtin - return from function with exit status
fn builtin_return(args: &[&str], _state: &mut ShellState<'_>) -> BuiltinResult {
    let code = if args.is_empty() {
        0
    } else {
        args[0].parse::<i32>().unwrap_or(0)
    };
    Err(BuiltinError::Return(code))
}

/// True builtin
#[allow(clippy::unnecessary_wraps)]
fn builtin_true(_args: &[&str], _state: &mut ShellState<'_>) -> BuiltinResult {
    Ok(super::executor::ExitStatus::SUCCESS)
}

/// False builtin
#[allow(clippy::unnecessary_wraps)]
fn builtin_false(_args: &[&str], _state: &mut ShellState<'_>) -> BuiltinResult {
    Ok(super::executor::ExitStatus {
        code: 1,
        signaled: false,
    })
}

/// Source builtin
fn builtin_source(args: &[&str], _state: &mut ShellState<'_>) -> BuiltinResult {
    if args.is_empty() {
        return Err(BuiltinError::Generic(
            "filename argument required".to_string(),
        ));
    }

    let path = args[0];
    // Verify file exists and is readable
    if !std::path::Path::new(path).exists() {
        return Err(BuiltinError::Generic(format!(
            "{path}: No such file or directory"
        )));
    }

    // Return Source error - the executor will handle actual execution
    Err(BuiltinError::Source(path.to_string()))
}

/// Alias builtin - list or define aliases
#[allow(clippy::unnecessary_wraps)]
fn builtin_alias(args: &[&str], state: &mut ShellState<'_>) -> BuiltinResult {
    if args.is_empty() {
        // List all aliases
        for (name, value) in state.aliases.iter() {
            println!("{}={}", name, shlex::quote(value));
        }
        return Ok(super::executor::ExitStatus::SUCCESS);
    }

    for arg in args {
        if let Some((name, value)) = arg.split_once('=') {
            // Define alias - strip matching outer quotes if present
            let value = if value.starts_with('\'') && value.ends_with('\'') && value.len() > 1 {
                // Strip single quotes
                value[1..value.len() - 1].to_string()
            } else if value.starts_with('"') && value.ends_with('"') && value.len() > 1 {
                // Strip double quotes
                value[1..value.len() - 1].to_string()
            } else {
                value.to_string()
            };
            state.aliases.insert(name.to_string(), value);
        } else {
            // Print specific alias
            if let Some(value) = state.aliases.get(*arg) {
                println!("{}={}", arg, shlex::quote(value));
            } else {
                return Err(BuiltinError::Generic(format!("alias: {arg}: not found")));
            }
        }
    }

    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Unalias builtin - remove aliases
#[allow(clippy::unnecessary_wraps)]
fn builtin_unalias(args: &[&str], state: &mut ShellState<'_>) -> BuiltinResult {
    if args.is_empty() {
        return Err(BuiltinError::Generic(
            "unalias: usage: unalias [-a] name [name ...]".to_string(),
        ));
    }

    if args[0] == "-a" {
        // Remove all aliases
        state.aliases.clear();
        return Ok(super::executor::ExitStatus::SUCCESS);
    }

    for arg in args {
        state.aliases.remove(*arg);
    }
    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Set builtin - set shell options and positional parameters
fn builtin_set(args: &[&str], state: &mut ShellState<'_>) -> BuiltinResult {
    if args.is_empty() {
        // Print all variables (same as export without args for now)
        for (k, v) in state.env.iter() {
            println!("{}={}", k, shlex::quote(v));
        }
        return Ok(super::executor::ExitStatus::SUCCESS);
    }

    // Check for -- to set positional parameters
    if args[0] == "--" {
        // Set positional parameters to remaining args
        state.positional_params.clear();
        for arg in &args[1..] {
            state.positional_params.push((*arg).to_string());
        }
        return Ok(super::executor::ExitStatus::SUCCESS);
    }

    // Handle options like -e, -x, -u, -f, etc.
    for arg in args {
        if arg.starts_with('-') && arg.len() > 1 {
            let opts = &arg[1..];
            for c in opts.chars() {
                match c {
                    'e' => state.options.errexit = true,
                    'x' => state.options.xtrace = true,
                    'u' => state.options.nounset = true,
                    'f' => state.options.noglob = true,
                    _ => {
                        return Err(BuiltinError::Generic(format!(
                            "set: invalid option: -{c}"
                        )))
                    }
                }
            }
        } else if arg.starts_with('+') && arg.len() > 1 {
            // Disable options with +e, +x, etc.
            let opts = &arg[1..];
            for c in opts.chars() {
                match c {
                    'e' => state.options.errexit = false,
                    'x' => state.options.xtrace = false,
                    'u' => state.options.nounset = false,
                    'f' => state.options.noglob = false,
                    _ => {
                        return Err(BuiltinError::Generic(format!(
                            "set: invalid option: +{c}"
                        )))
                    }
                }
            }
        } else {
            // Positional argument
            state.positional_params.push((*arg).to_string());
        }
    }

    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Shift builtin - shift positional parameters
fn builtin_shift(args: &[&str], state: &mut ShellState<'_>) -> BuiltinResult {
    let n = if args.is_empty() {
        1
    } else {
        args[0].parse::<usize>().map_err(|_| {
            BuiltinError::Generic(format!("shift: {}: numeric argument required", args[0]))
        })?
    };

    if n > state.positional_params.len() {
        return Err(BuiltinError::Generic(
            "shift: can't shift that many".to_string(),
        ));
    }

    // Remove first n elements
    state.positional_params.drain(0..n);

    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Test builtin - evaluate conditional expressions
/// Supports POSIX test operators: file tests, string tests, numeric tests
#[allow(clippy::unnecessary_wraps)]
fn builtin_test(args: &[&str], _state: &mut ShellState<'_>) -> BuiltinResult {
    // Handle [ ... ] syntax - check for closing ]
    let args = if args.last() == Some(&"]") {
        &args[..args.len() - 1]
    } else {
        args
    };

    let result = evaluate_test(args);

    if result {
        Ok(super::executor::ExitStatus::SUCCESS)
    } else {
        Ok(super::executor::ExitStatus {
            code: 1,
            signaled: false,
        })
    }
}

/// Evaluate test expression and return true/false
fn evaluate_test(args: &[&str]) -> bool {
    if args.is_empty() {
        return false;
    }

    // Handle negation: ! expr
    if args[0] == "!" {
        return !evaluate_test(&args[1..]);
    }

    // Handle single argument (check if non-empty string)
    if args.len() == 1 {
        return !args[0].is_empty();
    }

    // Handle two-argument tests
    if args.len() == 2 {
        let op = args[0];
        let operand = &args[1];
        return match op {
            "-n" => !operand.is_empty(),                     // non-zero length
            "-z" => operand.is_empty(),                      // zero length
            "-e" => std::path::Path::new(operand).exists(),  // file exists
            "-f" => std::path::Path::new(operand).is_file(), // regular file
            "-d" => std::path::Path::new(operand).is_dir(),  // directory
            "-r" => is_readable(operand),                    // readable
            "-w" => is_writable(operand),                    // writable
            "-x" => is_executable(operand),                  // executable
            "-s" => has_size(operand),                       // size > 0
            "-L" => is_symlink(operand),                     // is symlink
            _ => false,
        };
    }

    // Handle three-argument tests
    if args.len() == 3 {
        let left = args[0];
        let op = args[1];
        let right = args[2];

        return match op {
            "=" | "==" => left == right,                          // string equal
            "!=" => left != right,                                // string not equal
            "-eq" => compare_numeric(left, right, |a, b| a == b), // numeric equal
            "-ne" => compare_numeric(left, right, |a, b| a != b), // numeric not equal
            "-lt" => compare_numeric(left, right, |a, b| a < b),  // less than
            "-le" => compare_numeric(left, right, |a, b| a <= b), // less or equal
            "-gt" => compare_numeric(left, right, |a, b| a > b),  // greater than
            "-ge" => compare_numeric(left, right, |a, b| a >= b), // greater or equal
            "-a" => evaluate_test(&[left]) && evaluate_test(&[right]), // AND
            "-o" => evaluate_test(&[left]) || evaluate_test(&[right]), // OR
            _ => false,
        };
    }

    // Handle more complex expressions by finding -a or -o
    // This is a simplified version - full POSIX test is more complex
    for (i, arg) in args.iter().enumerate() {
        if *arg == "-a" && i > 0 && i < args.len() - 1 {
            let left = evaluate_test(&args[..i]);
            let right = evaluate_test(&args[i + 1..]);
            return left && right;
        }
    }

    false
}

/// Compare two numeric strings using the given comparison
fn compare_numeric<F>(left: &str, right: &str, cmp: F) -> bool
where
    F: Fn(i64, i64) -> bool,
{
    let left_val = left.parse::<i64>();
    let right_val = right.parse::<i64>();
    match (left_val, right_val) {
        (Ok(a), Ok(b)) => cmp(a, b),
        _ => false,
    }
}

/// Check if file is readable
fn is_readable(path: &str) -> bool {
    std::fs::metadata(path)
        .is_ok_and(|m| !m.permissions().readonly())
}

/// Check if file is writable
fn is_writable(path: &str) -> bool {
    // Simplified - on Unix would need access() syscall
    std::path::Path::new(path).exists()
}

/// Check if file is executable
fn is_executable(path: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        std::path::Path::new(path).exists()
    }
}

/// Check if file has size > 0
fn has_size(path: &str) -> bool {
    std::fs::metadata(path)
        .is_ok_and(|m| m.len() > 0)
}

/// Check if path is a symlink
fn is_symlink(path: &str) -> bool {
    std::fs::symlink_metadata(path)
        .is_ok_and(|m| m.file_type().is_symlink())
}

/// Read builtin - read a line from stdin into variable(s)
fn builtin_read(args: &[&str], state: &mut ShellState<'_>) -> BuiltinResult {
    use std::io::{self, BufRead};

    let mut prompt = String::new();
    let mut var_names: Vec<&str> = Vec::new();
    let mut has_prompt = false;

    // Parse options
    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-p" => {
                // Prompt option
                if i + 1 < args.len() {
                    prompt = args[i + 1].to_string();
                    has_prompt = true;
                    i += 2;
                } else {
                    return Err(BuiltinError::Generic(
                        "read: -p: option requires an argument".to_string(),
                    ));
                }
            }
            "-r" => {
                // Raw mode (no backslash escape interpretation) - we'll store but not implement fully
                i += 1;
            }
            "-s" => {
                // Silent mode (for passwords) - not fully implemented
                i += 1;
            }
            "-n" | "-t" => {
                // -n nchars, -t timeout - skip with their arguments
                if i + 1 < args.len() {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => {
                if args[i].starts_with('-') {
                    // Unknown option
                    return Err(BuiltinError::Generic(format!(
                        "read: invalid option: {}",
                        args[i]
                    )));
                }
                // This is a variable name
                var_names.push(args[i]);
                i += 1;
            }
        }
    }

    // Print prompt if provided
    if has_prompt {
        print!("{prompt}");
        let _ = io::Write::flush(&mut std::io::stdout());
    }

    // Read line from stdin
    let stdin = io::stdin();
    let mut line = String::new();
    match stdin.lock().read_line(&mut line) {
        Ok(0) => {
            // EOF reached - set variables to empty and return failure
            for name in &var_names {
                state.env.insert((*name).to_string(), String::new());
            }
            Ok(super::executor::ExitStatus {
                code: 1,
                signaled: false,
            })
        }
        Ok(_) => {
            // Remove trailing newline
            if line.ends_with('\n') {
                line.pop();
            }
            if line.ends_with('\r') {
                line.pop();
            }

            // Split into fields using IFS (simplified - just use whitespace)
            let fields: Vec<&str> = line.split_whitespace().collect();

            // Assign to variables
            if var_names.is_empty() {
                // No variable specified - store in REPLY
                state.env.insert("REPLY".to_string(), line);
            } else if var_names.len() == 1 {
                // Single variable - store entire line
                state.env.insert(var_names[0].to_string(), line);
            } else {
                // Multiple variables - split fields
                let last_idx = var_names.len() - 1;
                for (i, name) in var_names.iter().enumerate() {
                    if i < last_idx {
                        // Assign single field
                        let value = (*fields.get(i).unwrap_or(&"")).to_string();
                        state.env.insert((*name).to_string(), value);
                    } else {
                        // Last variable gets remaining fields
                        let remaining: Vec<&str> = fields.iter().skip(i).copied().collect();
                        state.env.insert((*name).to_string(), remaining.join(" "));
                    }
                }
            }

            Ok(super::executor::ExitStatus::SUCCESS)
        }
        Err(e) => Err(BuiltinError::Generic(format!("read: {e}"))),
    }
}

/// Trap builtin - set signal handlers
fn builtin_trap(args: &[&str], _state: &mut ShellState<'_>) -> BuiltinResult {
    #[cfg(not(unix))]
    {
        // Not supported on non-Unix platforms
        return Err(BuiltinError::Generic(
            "trap: signal handling not supported on this platform".to_string(),
        ));
    }

    #[cfg(unix)]
    {
        use libc::{signal, SIG_DFL, SIG_IGN};
        use std::collections::HashMap;

        // Signal name to number mapping (POSIX signals)
        let signals: HashMap<&str, i32> = [
            ("EXIT", 0),  // Special: exit handler
            ("HUP", 1),   // SIGHUP
            ("INT", 2),   // SIGINT
            ("QUIT", 3),  // SIGQUIT
            ("ILL", 4),   // SIGILL
            ("TRAP", 5),  // SIGTRAP
            ("ABRT", 6),  // SIGABRT
            ("FPE", 8),   // SIGFPE
            ("KILL", 9),  // SIGKILL
            ("BUS", 7),   // SIGBUS
            ("SEGV", 11), // SIGSEGV
            ("PIPE", 13), // SIGPIPE
            ("ALRM", 14), // SIGALRM
            ("TERM", 15), // SIGTERM
            ("USR1", 10), // SIGUSR1
            ("USR2", 12), // SIGUSR2
            ("CHLD", 17), // SIGCHLD
            ("CONT", 18), // SIGCONT
            ("STOP", 19), // SIGSTOP
            ("TSTP", 20), // SIGTSTP
            ("TTIN", 21), // SIGTTIN
            ("TTOU", 22), // SIGTTOU
        ]
        .iter()
        .copied()
        .collect();

        if args.is_empty() {
            // List current traps (not implemented - would need trap registry)
            return Ok(super::executor::ExitStatus::SUCCESS);
        }

        // Check for -l (list signals)
        if args[0] == "-l" {
            for name in signals.keys() {
                println!("{name})");
            }
            return Ok(super::executor::ExitStatus::SUCCESS);
        }

        // Parse action and signals
        let (action, signal_args) = if args.len() >= 2 && !args[0].starts_with('-') {
            // First arg is action (command or -/empty)
            (args[0], &args[1..])
        } else {
            // No action specified - print trap for given signals
            return Ok(super::executor::ExitStatus::SUCCESS);
        };

        // Process each signal
        for sig_arg in signal_args {
            // Parse signal number or name
            let sig_num = if let Ok(num) = sig_arg.parse::<i32>() {
                num
            } else {
                // Try to look up by name
                let name = sig_arg.strip_prefix("SIG").unwrap_or(sig_arg);
                match signals.get(name) {
                    Some(&num) => num,
                    None => {
                        return Err(BuiltinError::Generic(format!(
                            "trap: {sig_arg}: invalid signal specification"
                        )));
                    }
                }
            };

            // Apply action
            if action == "-" || action.is_empty() {
                // Reset to default
                unsafe { signal(sig_num, SIG_DFL) };
            } else if action == "''" || action == "\"\"" {
                // Ignore signal
                unsafe { signal(sig_num, SIG_IGN) };
            } else {
                // Custom command - would need signal handler registry
                // For now, just acknowledge the trap is set
            }
        }

        Ok(super::executor::ExitStatus::SUCCESS)
    }
}

/// Jobs builtin — list background jobs
fn builtin_jobs(args: &[&str], state: &mut ShellState<'_>) -> BuiltinResult {
    let Some(job_control) = state.job_control.as_mut() else {
        return Err(BuiltinError::Generic(
            "jobs: job control not available".to_string(),
        ));
    };

    let long_format = args.iter().any(|&a| a == "-l" || a == "-p");

    for job in job_control.list_jobs() {
        let status_str = match job.status {
            super::jobcontrol::JobStatus::Running => "Running",
            super::jobcontrol::JobStatus::Stopped => "Stopped",
            super::jobcontrol::JobStatus::Completed => "Done",
            super::jobcontrol::JobStatus::Killed => "Killed",
        };

        let current = if job_control.current_job() == Some(job.id) {
            "+"
        } else if job_control.previous_job() == Some(job.id) {
            "-"
        } else {
            " "
        };

        if long_format {
            if let Some(pgid) = job.pgid {
                println!(
                    "[{}] {} {} {} {}",
                    job.id, current, pgid, status_str, job.command
                );
            } else {
                println!("[{}] {} {} {}", job.id, current, status_str, job.command);
            }
        } else {
            println!("[{}] {} {} {}", job.id, current, status_str, job.command);
        }
    }

    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Parse a job specification string (e.g., "%1", "%%", "%-")
fn parse_job_spec(spec: &str, builtin_name: &str) -> Result<usize, String> {
    if let Some(inner) = spec.strip_prefix('%') {
        match inner {
            "%" | "" => Ok(0),     // current job (special value)
            "-" => Ok(usize::MAX), // previous job (special value)
            _ => inner
                .parse::<usize>()
                .map_err(|_| format!("{builtin_name}: {spec}: no such job")),
        }
    } else {
        spec.parse::<usize>()
            .map_err(|_| format!("{builtin_name}: {spec}: no such job"))
    }
}

/// Resolve job spec to actual job ID using job control
fn resolve_job_id(
    spec: &str,
    builtin_name: &str,
    job_control: &super::jobcontrol::JobControl,
) -> Result<usize, String> {
    let parsed = parse_job_spec(spec, builtin_name)?;
    if parsed == 0 {
        // Current job
        job_control
            .current_job()
            .ok_or_else(|| format!("{builtin_name}: no current job"))
    } else if parsed == usize::MAX {
        // Previous job
        job_control
            .previous_job()
            .ok_or_else(|| format!("{builtin_name}: no previous job"))
    } else {
        // Explicit job ID
        if job_control.get_job(parsed).is_some() {
            Ok(parsed)
        } else {
            Err(format!("{builtin_name}: %{parsed}: no such job"))
        }
    }
}

/// Foreground builtin — bring job to foreground
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
fn builtin_fg(args: &[&str], state: &mut ShellState<'_>) -> BuiltinResult {
    let Some(job_control) = state.job_control.as_mut() else {
        return Err(BuiltinError::Generic(
            "fg: job control not available".to_string(),
        ));
    };

    let job_spec = if args.is_empty() { "%" } else { args[0] };
    let job_id = resolve_job_id(job_spec, "fg", job_control).map_err(BuiltinError::Generic)?;

    #[cfg(unix)]
    {
        match job_control.foreground(job_id) {
            Ok(status) => Ok(super::executor::ExitStatus {
                code: status.clamp(0, 255) as u8,
                signaled: status > 128,
            }),
            Err(e) => Err(BuiltinError::Generic(e)),
        }
    }

    #[cfg(not(unix))]
    {
        Err(BuiltinError::Generic(
            "fg: job control not supported on this platform".to_string(),
        ))
    }
}

/// Background builtin — continue stopped job in background
fn builtin_bg(args: &[&str], state: &mut ShellState<'_>) -> BuiltinResult {
    let Some(job_control) = state.job_control.as_mut() else {
        return Err(BuiltinError::Generic(
            "bg: job control not available".to_string(),
        ));
    };

    let job_spec = if args.is_empty() { "%" } else { args[0] };
    let job_id = resolve_job_id(job_spec, "bg", job_control).map_err(BuiltinError::Generic)?;

    #[cfg(unix)]
    {
        match job_control.background(job_id) {
            Ok(()) => Ok(super::executor::ExitStatus::SUCCESS),
            Err(e) => Err(BuiltinError::Generic(e)),
        }
    }

    #[cfg(not(unix))]
    {
        Err(BuiltinError::Generic(
            "bg: job control not supported on this platform".to_string(),
        ))
    }
}

// Simple quote helper
mod shlex {
    pub fn quote(s: &str) -> String {
        if s.contains(' ') || s.contains('\t') || s.contains('\n') || s.contains('"') {
            format!("'{}'", s.replace('\'', "'\"'\"'"))
        } else {
            s.to_string()
        }
    }
}
