//! Context retriever — Pull relevant context for current input

use crate::context::{CommandNode, ContextGraph, ProjectNode};
use std::fmt::Write;
use thiserror::Error;

/// Retriever errors
#[derive(Error, Debug)]
pub enum RetrieveError {
    /// Database error
    #[error("database error: {0}")]
    Database(#[from] crate::context::ContextError),
    /// No context available
    #[error("no context available")]
    NoContext,
}

/// Retrieved context
#[derive(Debug, Clone)]
pub struct RetrievedContext {
    /// Relevant commands
    pub commands: Vec<CommandNode>,
    /// Current project info
    pub project: Option<ProjectNode>,
    /// Context summary for LLM
    pub summary: String,
}

/// Context retriever
pub struct ContextRetriever<'a> {
    graph: &'a ContextGraph,
    max_commands: usize,
    max_context_length: usize,
}

impl<'a> ContextRetriever<'a> {
    /// Create a new retriever
    pub fn new(graph: &'a ContextGraph) -> Self {
        Self {
            graph,
            max_commands: 10,
            max_context_length: 4000,
        }
    }

    /// Set max commands to retrieve
    #[must_use]
    pub fn with_max_commands(mut self, n: usize) -> Self {
        self.max_commands = n;
        self
    }

    /// Retrieve context for the current input
    ///
    /// # Errors
    /// Returns an error if context retrieval fails
    pub fn retrieve(
        &self,
        _input: &str,
        current_dir: &std::path::Path,
    ) -> Result<RetrievedContext, RetrieveError> {
        // Find current project
        let project = Self::find_project(current_dir);

        // Find relevant commands
        let mut commands = Vec::new();

        // 1. Commands from the same directory
        // TODO: Query by directory

        // 2. Commands from the same project
        if let Some(ref p) = project {
            let project_cmds = self
                .graph
                .query_project_commands(&p.id, self.max_commands)?;
            commands.extend(project_cmds);
        }

        // 3. Commands matching the input prefix
        // TODO: Fuzzy match against command history

        // 4. Most recent commands
        // (already included from project query)

        // Build summary
        let summary = self.build_summary(&commands, project.as_ref());

        Ok(RetrievedContext {
            commands,
            project,
            summary,
        })
    }

    #[allow(clippy::unnecessary_wraps)]
    fn find_project(dir: &std::path::Path) -> Option<ProjectNode> {
        // Walk up the directory tree looking for project markers
        let mut current = Some(dir);

        while let Some(path) = current {
            // Check for common project markers
            let markers = [
                "Cargo.toml",
                "package.json",
                "pyproject.toml",
                "setup.py",
                "Makefile",
                ".git",
            ];

            for marker in &markers {
                if path.join(marker).exists() {
                    // Found a project, get or create it
                    let project_type = match *marker {
                        "Cargo.toml" => "rust",
                        "package.json" => "node",
                        "pyproject.toml" | "setup.py" => "python",
                        "Makefile" => "c",
                        ".git" => "git",
                        _ => "unknown",
                    };

                    return Some(ProjectNode {
                        id: format!("project:{}", path.display()),
                        path: path.to_path_buf(),
                        project_type: project_type.to_string(),
                        stack: Vec::new(),
                        git_remote: None,
                        created: std::time::SystemTime::now(),
                        last_accessed: std::time::SystemTime::now(),
                        access_count: 1,
                    });
                }
            }

            current = path.parent();
        }

        None
    }

    fn build_summary(&self, commands: &[CommandNode], project: Option<&ProjectNode>) -> String {
        let mut summary = String::new();

        if let Some(p) = project {
            let _ = writeln!(
                summary,
                "Project: {} ({} at {})",
                p.path.file_name().unwrap_or_default().to_string_lossy(),
                p.project_type,
                p.path.display()
            );
        }

        if !commands.is_empty() {
            summary.push_str("Recent commands:\n");
            for cmd in commands.iter().take(self.max_commands) {
                let _ = writeln!(summary, "  - {} (exit: {})", cmd.command, cmd.exit_code);
            }
        }

        summary
    }

    /// Cap context to fit in LLM context window
    #[must_use]
    pub fn cap_context(&self, context: &str) -> String {
        if context.len() <= self.max_context_length {
            context.to_string()
        } else {
            // Truncate but try to keep command boundaries
            let truncate_at = context[..self.max_context_length]
                .rfind('\n')
                .unwrap_or(self.max_context_length);
            format!("{}\n... (truncated)", &context[..truncate_at])
        }
    }
}
