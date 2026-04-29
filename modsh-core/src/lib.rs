//! modsh-core — POSIX-compatible shell core
//!
//! This crate provides the foundational shell functionality:
//! - Lexer for tokenizing POSIX shell syntax
//! - Parser for building AST from tokens
//! - Expander for variable, glob, and command expansion
//! - Executor for forking/execing commands
//! - Builtins for shell builtin commands
//! - Job control for foreground/background execution

#![warn(missing_docs)]

pub mod builtins;
pub mod executor;
pub mod expander;
pub mod jobcontrol;
pub mod lexer;
pub mod parser;
