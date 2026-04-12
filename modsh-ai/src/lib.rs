//! modsh-ai — AI context engine
//!
//! This crate provides local-first AI features:
//! - Context graph for storing command history and patterns
//! - Local inference via Ollama or llama.cpp
//! - Context retrieval for relevant suggestions
//! - Feedback loop for learning from accept/reject
//!
//! License: BSL 1.1

#![warn(missing_docs)]

pub mod context;
pub mod inference;
pub mod retriever;
pub mod feedback;
