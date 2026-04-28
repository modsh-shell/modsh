//! Completion engine — zsh-style tab completion

use std::path::PathBuf;

/// A completion candidate
#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    /// The text to insert
    pub text: String,
    /// Description of the completion
    pub description: Option<String>,
    /// Type of completion
    pub kind: CompletionKind,
}

/// Kind of completion
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompletionKind {
    /// Command name
    Command,
    /// File path
    File,
    /// Directory path
    Directory,
    /// Variable name
    Variable,
    /// Option flag
    Flag,
    /// Other
    Other,
}

/// Completion context
#[derive(Debug, Clone)]
pub struct CompletionContext {
    /// The partial word being completed
    pub word: String,
    /// The full line being edited
    pub line: String,
    /// Position in the line
    pub position: usize,
    /// Whether completing a command (first word)
    pub is_command: bool,
}

/// Completion engine
pub struct CompletionEngine;

impl CompletionEngine {
    /// Create a new completion engine
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Get completions for the given context
    #[must_use]
    pub fn complete(&self, ctx: &CompletionContext) -> Vec<Completion> {
        if ctx.is_command {
            Self::complete_commands(&ctx.word)
        } else if ctx.word.starts_with('$') {
            Self::complete_variables(&ctx.word)
        } else if ctx.word.starts_with('-') {
            Self::complete_flags(&ctx.word)
        } else {
            Self::complete_paths(&ctx.word)
        }
    }

    fn complete_commands(prefix: &str) -> Vec<Completion> {
        let mut results = Vec::new();

        // Add builtins
        let builtins = [
            "cd", "pwd", "echo", "export", "unset", "env", "exit", "true", "false", "source", ".",
            "alias", "unalias", "read", "test", "[", "trap", "shift", "set", "return", "jobs",
            "fg", "bg",
        ];

        for cmd in &builtins {
            if cmd.starts_with(prefix) {
                results.push(Completion {
                    text: (*cmd).to_string(),
                    description: Some("builtin".to_string()),
                    kind: CompletionKind::Command,
                });
            }
        }

        // Add commands from PATH
        if let Ok(path) = std::env::var("PATH") {
            for dir in path.split(':') {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.starts_with(prefix) && !results.iter().any(|c| c.text == name) {
                                results.push(Completion {
                                    text: name.to_string(),
                                    description: None,
                                    kind: CompletionKind::Command,
                                });
                            }
                        }
                    }
                }
            }
        }

        results.sort_by(|a, b| a.text.cmp(&b.text));
        results.dedup_by(|a, b| a.text == b.text);

        results
    }

    fn complete_paths(prefix: &str) -> Vec<Completion> {
        let mut results = Vec::new();

        let (dir_part, file_prefix) = if prefix.contains('/') {
            let idx = prefix.rfind('/').unwrap();
            (&prefix[..=idx], &prefix[idx + 1..])
        } else {
            ("./", prefix)
        };

        let dir_path = if let Some(stripped) = dir_part.strip_prefix("~/") {
            let home = std::env::var("HOME").unwrap_or_default();
            PathBuf::from(home).join(stripped)
        } else {
            PathBuf::from(dir_part)
        };

        if let Ok(entries) = std::fs::read_dir(&dir_path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                if name_str.starts_with(file_prefix) {
                    let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
                    let text = if dir_part == "./" {
                        name_str.to_string()
                    } else {
                        format!("{dir_part}{name_str}")
                    };

                    let text = if is_dir { format!("{text}/") } else { text };

                    results.push(Completion {
                        text,
                        description: None,
                        kind: if is_dir {
                            CompletionKind::Directory
                        } else {
                            CompletionKind::File
                        },
                    });
                }
            }
        }

        results.sort_by(|a, b| {
            // Directories first
            let a_dir = matches!(a.kind, CompletionKind::Directory);
            let b_dir = matches!(b.kind, CompletionKind::Directory);
            b_dir.cmp(&a_dir).then_with(|| a.text.cmp(&b.text))
        });

        results
    }

    fn complete_variables(prefix: &str) -> Vec<Completion> {
        let mut results = Vec::new();
        let var_prefix = &prefix[1..]; // Remove $

        for (key, _) in std::env::vars() {
            if key.starts_with(var_prefix) {
                results.push(Completion {
                    text: format!("${key}"),
                    description: None,
                    kind: CompletionKind::Variable,
                });
            }
        }

        results.sort_by(|a, b| a.text.cmp(&b.text));
        results
    }

    fn complete_flags(_prefix: &str) -> Vec<Completion> {
        // TODO: Parse --help to get flags
        Vec::new()
    }
}

impl Default for CompletionEngine {
    fn default() -> Self {
        Self::new()
    }
}
