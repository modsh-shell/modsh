//! Builtins — Shell builtin commands

use std::collections::HashMap;
use thiserror::Error;

/// Errors from builtin commands
#[derive(Error, Debug)]
pub enum BuiltinError {
    #[error("{0}")]
    Generic(String),
    #[error("exit {0}")]
    Exit(i32),
    #[error("return {0}")]
    Return(i32),
}

/// Result type for builtins
pub type BuiltinResult = Result<super::executor::ExitStatus, BuiltinError>;

/// Builtin function type
pub type BuiltinFn = fn(&[&str], &mut HashMap<String, String>) -> BuiltinResult;

/// Get a builtin by name
pub fn get_builtin(name: &str) -> Option<BuiltinFn> {
    match name {
        "cd" => Some(builtin_cd),
        "pwd" => Some(builtin_pwd),
        "echo" => Some(builtin_echo),
        "export" => Some(builtin_export),
        "unset" => Some(builtin_unset),
        "env" => Some(builtin_env),
        "exit" => Some(builtin_exit),
        "true" => Some(builtin_true),
        "false" => Some(builtin_false),
        "." | "source" => Some(builtin_source),
        _ => None,
    }
}

/// Change directory builtin
fn builtin_cd(args: &[&str], env: &mut HashMap<String, String>) -> BuiltinResult {
    let path = if args.is_empty() {
        // Go to HOME
        std::env::var("HOME").map_err(|_| {
            BuiltinError::Generic("HOME not set".to_string())
        })?
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

    std::env::set_current_dir(&new_path)
        .map_err(|e| BuiltinError::Generic(e.to_string()))?;

    // Update PWD
    let canonical = new_path.canonicalize()
        .unwrap_or(new_path)
        .to_string_lossy()
        .to_string();
    env.insert("PWD".to_string(), canonical.clone());
    env.insert("OLDPWD".to_string(), env.get("PWD").cloned().unwrap_or_default());

    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Print working directory builtin
fn builtin_pwd(_args: &[&str], _env: &mut HashMap<String, String>) -> BuiltinResult {
    let cwd = std::env::current_dir()
        .map_err(|e| BuiltinError::Generic(e.to_string()))?;
    println!("{}", cwd.display());
    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Echo builtin
fn builtin_echo(args: &[&str], _env: &mut HashMap<String, String>) -> BuiltinResult {
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
        println!("{}", output);
    } else {
        print!("{}", output);
    }

    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Export builtin
fn builtin_export(args: &[&str], env: &mut HashMap<String, String>) -> BuiltinResult {
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
fn builtin_unset(args: &[&str], env: &mut HashMap<String, String>) -> BuiltinResult {
    for arg in args {
        env.remove(*arg);
        std::env::remove_var(arg);
    }
    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Env builtin - print environment
fn builtin_env(_args: &[&str], _env: &mut HashMap<String, String>) -> BuiltinResult {
    for (k, v) in std::env::vars() {
        println!("{}={}", k, v);
    }
    Ok(super::executor::ExitStatus::SUCCESS)
}

/// Exit builtin
fn builtin_exit(args: &[&str], _env: &mut HashMap<String, String>) -> BuiltinResult {
    let code = if args.is_empty() {
        0
    } else {
        args[0].parse::<i32>().unwrap_or(0)
    };
    Err(BuiltinError::Exit(code))
}

/// True builtin
fn builtin_true(_args: &[&str], _env: &mut HashMap<String, String>) -> BuiltinResult {
    Ok(super::executor::ExitStatus::SUCCESS)
}

/// False builtin
fn builtin_false(_args: &[&str], _env: &mut HashMap<String, String>) -> BuiltinResult {
    Ok(super::executor::ExitStatus { code: 1, signaled: false })
}

/// Source builtin
fn builtin_source(args: &[&str], _env: &mut HashMap<String, String>) -> BuiltinResult {
    if args.is_empty() {
        return Err(BuiltinError::Generic("filename argument required".to_string()));
    }

    let path = args[0];
    let _content = std::fs::read_to_string(path)
        .map_err(|e| BuiltinError::Generic(e.to_string()))?;

    // TODO: Execute the script content
    // For now, just return success
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
