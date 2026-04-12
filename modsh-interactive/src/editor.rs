//! Line editor — Cursor movement, history, multi-line editing

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use std::io::{self, Write};

/// Line editor state
pub struct LineEditor {
    buffer: String,
    cursor: usize,
    history: Vec<String>,
    history_index: Option<usize>,
    multiline: bool,
}

impl LineEditor {
    /// Create a new line editor
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            history: Vec::new(),
            history_index: None,
            multiline: false,
        }
    }

    /// Read a line from the user
    pub fn read_line(&mut self, prompt: &str) -> io::Result<String> {
        print!("{}", prompt);
        io::stdout().flush()?;

        // TODO: Implement proper raw mode with crossterm
        // For now, use standard input
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        
        // Remove trailing newline
        if line.ends_with('\n') {
            line.pop();
        }
        if line.ends_with('\r') {
            line.pop();
        }
        
        self.history.push(line.clone());
        
        Ok(line)
    }

    /// Handle a key event
    fn handle_key(&mut self, key: KeyEvent) -> Option<ReadResult> {
        match key.code {
            KeyCode::Enter => {
                if self.multiline {
                    self.buffer.push('\n');
                    self.cursor += 1;
                    None
                } else {
                    Some(ReadResult::Line(self.buffer.clone()))
                }
            }
            KeyCode::Char(c) => {
                self.buffer.insert(self.cursor, c);
                self.cursor += 1;
                None
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.buffer.remove(self.cursor);
                }
                None
            }
            KeyCode::Delete => {
                if self.cursor < self.buffer.len() {
                    self.buffer.remove(self.cursor);
                }
                None
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                None
            }
            KeyCode::Right => {
                if self.cursor < self.buffer.len() {
                    self.cursor += 1;
                }
                None
            }
            KeyCode::Home => {
                self.cursor = 0;
                None
            }
            KeyCode::End => {
                self.cursor = self.buffer.len();
                None
            }
            KeyCode::Up => {
                self.history_up();
                None
            }
            KeyCode::Down => {
                self.history_down();
                None
            }
            KeyCode::Esc => Some(ReadResult::Cancel),
            KeyCode::Tab => None, // Handled by completion
            _ => None,
        }
    }

    fn history_up(&mut self) {
        if let Some(idx) = self.history_index {
            if idx > 0 {
                self.history_index = Some(idx - 1);
                self.buffer = self.history[idx - 1].clone();
                self.cursor = self.buffer.len();
            }
        } else if !self.history.is_empty() {
            self.history_index = Some(self.history.len() - 1);
            self.buffer = self.history[self.history.len() - 1].clone();
            self.cursor = self.buffer.len();
        }
    }

    fn history_down(&mut self) {
        if let Some(idx) = self.history_index {
            if idx + 1 < self.history.len() {
                self.history_index = Some(idx + 1);
                self.buffer = self.history[idx + 1].clone();
                self.cursor = self.buffer.len();
            } else {
                self.history_index = None;
                self.buffer.clear();
                self.cursor = 0;
            }
        }
    }

    /// Get the current buffer content
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Get the cursor position
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.history_index = None;
    }

    /// Set multiline mode
    pub fn set_multiline(&mut self, multiline: bool) {
        self.multiline = multiline;
    }

    /// Add to history
    pub fn add_history(&mut self, line: String) {
        if !line.trim().is_empty() {
            self.history.push(line);
        }
    }
}

impl Default for LineEditor {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of reading a line
#[derive(Debug, Clone, PartialEq)]
pub enum ReadResult {
    /// A complete line was entered
    Line(String),
    /// Input was cancelled (e.g., Ctrl+C)
    Cancel,
    /// EOF was reached (e.g., Ctrl+D)
    Eof,
}
