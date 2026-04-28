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
#![allow(
    clippy::match_same_arms,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::unused_self,
    clippy::unnecessary_wraps,
    clippy::for_kv_map,
    clippy::manual_let_else,
    clippy::self_only_used_in_recursion,
    clippy::manual_strip
)]

pub mod builtins;
pub mod executor;
pub mod expander;
pub mod jobcontrol;
pub mod lexer;
pub mod parser;
