//! History engine — Structured history with metadata

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;

/// A history entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// The command
    pub command: String,
    /// Working directory when command was run
    pub directory: PathBuf,
    /// Exit code
    pub exit_code: u8,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Timestamp
    pub timestamp: SystemTime,
}

/// History engine
pub struct HistoryEngine {
    entries: Vec<HistoryEntry>,
    dedup: bool,
    max_size: usize,
    history_file: Option<PathBuf>,
}

impl HistoryEngine {
    /// Create a new history engine
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            dedup: true,
            max_size: 50000,
            history_file: None,
        }
    }

    /// Add an entry to history
    pub fn add(&mut self, entry: HistoryEntry) {
        // Deduplication: remove existing identical command
        if self.dedup {
            self.entries.retain(|e| e.command != entry.command);
        }

        self.entries.push(entry);

        // Trim to max size
        if self.entries.len() > self.max_size {
            let excess = self.entries.len() - self.max_size;
            self.entries.drain(0..excess);
        }
    }

    /// Add a command (with current metadata)
    pub fn add_command(&mut self, command: String, exit_code: u8, duration_ms: u64) {
        let entry = HistoryEntry {
            command,
            directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            exit_code,
            duration_ms,
            timestamp: SystemTime::now(),
        };
        self.add(entry);
    }

    /// Search history with a query
    pub fn search(&self, query: &str) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.command.contains(query))
            .collect()
    }

    /// Search with fuzzy matching
    pub fn fuzzy_search(&self, query: &str) -> Vec<&HistoryEntry> {
        // Simple substring matching for now
        // TODO: Implement proper fuzzy matching
        self.search(query)
    }

    /// Filter by directory
    pub fn filter_by_directory(&self, dir: &PathBuf) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.directory == *dir)
            .collect()
    }

    /// Get recent entries
    pub fn recent(&self, n: usize) -> Vec<&HistoryEntry> {
        self.entries.iter().rev().take(n).collect()
    }

    /// Get all entries
    pub fn all(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Set the history file path
    pub fn set_history_file(&mut self, path: PathBuf) {
        self.history_file = Some(path);
    }

    /// Load history from file
    pub fn load(&mut self) -> Result<(), std::io::Error> {
        if let Some(ref path) = self.history_file {
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                // TODO: Parse the history format
                // For now, simple line-based
                for line in content.lines() {
                    if !line.is_empty() {
                        self.add_command(line.to_string(), 0, 0);
                    }
                }
            }
        }
        Ok(())
    }

    /// Save history to file
    pub fn save(&self) -> Result<(), std::io::Error> {
        if let Some(ref path) = self.history_file {
            let mut content = String::new();
            for entry in &self.entries {
                content.push_str(&entry.command);
                content.push('\n');
            }
            std::fs::write(path, content)?;
        }
        Ok(())
    }

    /// Export history to a file
    pub fn export(&self, path: &PathBuf) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(&self.entries)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Import history from a file
    pub fn import(&mut self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let entries: Vec<HistoryEntry> = serde_json::from_str(&content)?;
        self.entries.extend(entries);
        Ok(())
    }
}

impl Default for HistoryEngine {
    fn default() -> Self {
        Self::new()
    }
}
