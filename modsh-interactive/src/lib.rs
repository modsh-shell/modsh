//! modsh-interactive — Extended interactive layer
//!
//! This crate provides modern interactive shell features:
//! - Line editor with cursor movement, kill/yank, history search
//! - Syntax highlighting with real-time token coloring
//! - Autosuggestions with fish-style ghost text
//! - Tab completion with descriptions
//! - Prompt engine with async rendering
//! - History engine with metadata
//! - Plugin system with WASM sandbox

#![warn(missing_docs)]

pub mod autosuggest;
pub mod complete;
pub mod editor;
pub mod highlight;
pub mod history;
pub mod plugin;
pub mod prompt;
