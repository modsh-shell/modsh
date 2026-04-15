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

/// Builtin function type
pub type BuiltinFn = fn(&[&str], &mut HashMap<String, String>, &mut HashMap<String, String>) -> BuiltinResult;

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
        "true" => Some(builtin_true),
        "false" => Some(builtin_false),
        "alias" => Some(builtin_alias),
        "unalias" => Some(builtin_unalias),
        "." | "source" => Some(builtin_source),
        _ => None,
    }
}

/// Change directory builtin
fn builtin_cd(args: &[&str], env: &mut HashMap<String, String>, _aliases: &mut HashMap<String, String>) -> BuiltinResult {
    let path = if args.is_empty() {
        // Go to HOME
        std::env::var("HOME").map_err(|_| BuiltinError::Generic("HOME not set".to_string()))?
    } else {
        args[0].to_string()
    };

    let new_path = if path.starts_with('/') {
        std::path::PathBuf::from(path)
    } else {
        std::env::current_dir()
            .map_err(|e| BuiltinError::Generic(e.to_string()))?
            .join(path)
    };

    // Get current PWD before changing directory (for OLDPWD)
    let old_pwd = env.get("PWD").cloned().unwrap_or_default();

    std::env::set_current_dir(&new_path).map_err(|e| BuiltinError::Generic(e.to_string()))?;

    // Update PWD
    let canonical = new_path
        .canonicalize()
        .unwrap_or(new_path)
        .to_string_lossy()
        .to_string();

    // Set OLDPWD first (to the previous PWD), then update PWD
    env.insert("OLDPWD".to_string(), old_pwd);
    env.insert("PWD".to_string(), canonical);

    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Print working directory builtin
fn builtin_pwd(_args: &[&str], _env: &mut HashMap<String, String>, _aliases: &mut HashMap<String, String>) -> BuiltinResult {
    let cwd = std::env::current_dir().map_err(|e| BuiltinError::Generic(e.to_string()))?;
    println!("{}", cwd.display());
    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Echo builtin
#[allow(clippy::unnecessary_wraps)]
fn builtin_echo(args: &[&str], _env: &mut HashMap<String, String>, _aliases: &mut HashMap<String, String>) -> BuiltinResult {
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
fn builtin_printf(args: &[&str], _env: &mut HashMap<String, String>, _aliases: &mut HashMap<String, String>) -> BuiltinResult {
    if args.is_empty() {
        return Err(BuiltinError::Generic("printf: missing format string".to_string()));
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
            let mut format_char = '\0';

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
            format_char = chars.next().unwrap_or('\0');

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
                    let s = format!("{:o}", val);
                    output.push_str(&apply_width(&s, width, left_align));
                    arg_idx += 1;
                }
                'x' => {
                    // Unsigned hex lowercase
                    let arg = args.get(arg_idx).unwrap_or(&"0");
                    let val = arg.parse::<u64>().unwrap_or(0);
                    let s = format!("{:x}", val);
                    output.push_str(&apply_width(&s, width, left_align));
                    arg_idx += 1;
                }
                'X' => {
                    // Unsigned hex uppercase
                    let arg = args.get(arg_idx).unwrap_or(&"0");
                    let val = arg.parse::<u64>().unwrap_or(0);
                    let s = format!("{:X}", val);
                    output.push_str(&apply_width(&s, width, left_align));
                    arg_idx += 1;
                }
                'f' => {
                    // Floating point
                    let arg = args.get(arg_idx).unwrap_or(&"0");
                    let val = arg.parse::<f64>().unwrap_or(0.0);
                    let prec = precision.unwrap_or(6);
                    let s = format!("{:.*}", prec, val);
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
                Some('\\') => output.push('\\'),
                Some('a') => output.push('\x07'), // BEL
                Some('b') => output.push('\x08'), // BS
                Some('f') => output.push('\x0C'), // FF
                Some('v') => output.push('\x0B'), // VT
                Some(ch) => {
                    output.push('\\');
                    output.push(ch);
                }
                None => output.push('\\'),
            }
        } else {
            output.push(ch);
        }
    }

    print!("{}", output);
    let _ = std::io::Write::flush(&mut std::io::stdout());

    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Apply width padding to a string
fn apply_width(s: &str, width: Option<usize>, left_align: bool) -> String {
    match width {
        Some(w) if s.len() < w => {
            let padding = " ".repeat(w - s.len());
            if left_align {
                format!("{}{}", s, padding)
            } else {
                format!("{}{}", padding, s)
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
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('\\') => result.push('\\'),
                Some('a') => result.push('\x07'),
                Some('b') => result.push('\x08'),
                Some('f') => result.push('\x0C'),
                Some('v') => result.push('\x0B'),
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
                None => result.push('\\'),
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Export builtin
#[allow(clippy::unnecessary_wraps)]
fn builtin_export(args: &[&str], env: &mut HashMap<String, String>, _aliases: &mut HashMap<String, String>) -> BuiltinResult {
    if args.is_empty() {
        // Print all exported variables
        for (k, v) in env.iter() {
            println!("export {}={}", k, shlex::quote(v));
        }
        return Ok(super::executor::ExitStatus::SUCCESS);
    }

    for arg in args {
        if let Some((name, value)) = arg.split_once('=') {
            env.insert(name.to_string(), value.to_string());
            std::env::set_var(name, value);
        } else {
            // Mark existing variable as exported (already in env HashMap)
            if let Some(value) = env.get(*arg).cloned() {
                std::env::set_var(arg, value);
            }
        }
    }

    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Unset builtin
#[allow(clippy::unnecessary_wraps)]
fn builtin_unset(args: &[&str], env: &mut HashMap<String, String>, _aliases: &mut HashMap<String, String>) -> BuiltinResult {
    for arg in args {
        env.remove(*arg);
        std::env::remove_var(arg);
    }
    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Env builtin - print environment
#[allow(clippy::unnecessary_wraps)]
fn builtin_env(_args: &[&str], _env: &mut HashMap<String, String>, _aliases: &mut HashMap<String, String>) -> BuiltinResult {
    for (k, v) in std::env::vars() {
        println!("{k}={v}");
    }
    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Exit builtin
fn builtin_exit(args: &[&str], _env: &mut HashMap<String, String>, _aliases: &mut HashMap<String, String>) -> BuiltinResult {
    let code = if args.is_empty() {
        0
    } else {
        args[0].parse::<i32>().unwrap_or(0)
    };
    Err(BuiltinError::Exit(code))
}

/// True builtin
#[allow(clippy::unnecessary_wraps)]
fn builtin_true(_args: &[&str], _env: &mut HashMap<String, String>, _aliases: &mut HashMap<String, String>) -> BuiltinResult {
    Ok(super::executor::ExitStatus::SUCCESS)
}

/// False builtin
#[allow(clippy::unnecessary_wraps)]
fn builtin_false(_args: &[&str], _env: &mut HashMap<String, String>, _aliases: &mut HashMap<String, String>) -> BuiltinResult {
    Ok(super::executor::ExitStatus {
        code: 1,
        signaled: false,
    })
}

/// Source builtin
fn builtin_source(args: &[&str], _env: &mut HashMap<String, String>, _aliases: &mut HashMap<String, String>) -> BuiltinResult {
    if args.is_empty() {
        return Err(BuiltinError::Generic(
            "filename argument required".to_string(),
        ));
    }

    let path = args[0];
    // Verify file exists and is readable
    if !std::path::Path::new(path).exists() {
        return Err(BuiltinError::Generic(format!("{}: No such file or directory", path)));
    }

    // Return Source error - the executor will handle actual execution
    Err(BuiltinError::Source(path.to_string()))
}

/// Alias builtin - list or define aliases
#[allow(clippy::unnecessary_wraps)]
fn builtin_alias(args: &[&str], _env: &mut HashMap<String, String>, aliases: &mut HashMap<String, String>) -> BuiltinResult {
    if args.is_empty() {
        // List all aliases
        for (name, value) in aliases.iter() {
            println!("{}={}", name, shlex::quote(value));
        }
        return Ok(super::executor::ExitStatus::SUCCESS);
    }

    for arg in args {
        if let Some((name, value)) = arg.split_once('=') {
            // Define alias - strip matching outer quotes if present
            let value = if value.starts_with('\'') && value.ends_with('\'') && value.len() > 1 {
                // Strip single quotes
                value[1..value.len()-1].to_string()
            } else if value.starts_with('"') && value.ends_with('"') && value.len() > 1 {
                // Strip double quotes
                value[1..value.len()-1].to_string()
            } else {
                value.to_string()
            };
            aliases.insert(name.to_string(), value);
        } else {
            // Print specific alias
            if let Some(value) = aliases.get(*arg) {
                println!("{}={}", arg, shlex::quote(value));
            } else {
                return Err(BuiltinError::Generic(format!("alias: {}: not found", arg)));
            }
        }
    }

    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Unalias builtin - remove aliases
#[allow(clippy::unnecessary_wraps)]
fn builtin_unalias(args: &[&str], _env: &mut HashMap<String, String>, aliases: &mut HashMap<String, String>) -> BuiltinResult {
    if args.is_empty() {
        return Err(BuiltinError::Generic("unalias: usage: unalias [-a] name [name ...]".to_string()));
    }

    if args[0] == "-a" {
        // Remove all aliases
        aliases.clear();
        return Ok(super::executor::ExitStatus::SUCCESS);
    }

    for name in args {
        if aliases.remove(*name).is_none() {
            return Err(BuiltinError::Generic(format!("unalias: {}: no such alias", name)));
        }
    }

    Ok(super::executor::ExitStatus::SUCCESS)
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
