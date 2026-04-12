//! Feedback loop — Accept/reject tracking and weight adjustment

use crate::context::ContextGraph;
use std::time::SystemTime;

/// Feedback record
#[derive(Debug, Clone)]
pub struct Feedback {
    /// Suggested command
    pub suggestion: String,
    /// User input that triggered the suggestion
    pub context: String,
    /// Whether the user accepted it
    pub accepted: bool,
    /// Timestamp
    pub timestamp: SystemTime,
    /// Time to accept/reject (ms)
    pub decision_time_ms: u64,
}

/// Feedback collector
pub struct FeedbackCollector {
    pending: Option<SuggestionState>,
    history: Vec<Feedback>,
}

/// State of a pending suggestion
#[derive(Debug, Clone)]
struct SuggestionState {
    suggestion: String,
    context: String,
    shown_at: SystemTime,
}

impl FeedbackCollector {
    /// Create a new feedback collector
    pub fn new() -> Self {
        Self {
            pending: None,
            history: Vec::new(),
        }
    }

    /// Record that a suggestion was shown
    pub fn suggestion_shown(&mut self, suggestion: String, context: String) {
        self.pending = Some(SuggestionState {
            suggestion,
            context,
            shown_at: SystemTime::now(),
        });
    }

    /// Record that a suggestion was accepted
    pub fn accepted(&mut self) {
        if let Some(state) = self.pending.take() {
            let decision_time = SystemTime::now()
                .duration_since(state.shown_at)
                .unwrap_or_default()
                .as_millis() as u64;

            self.history.push(Feedback {
                suggestion: state.suggestion,
                context: state.context,
                accepted: true,
                timestamp: SystemTime::now(),
                decision_time_ms: decision_time,
            });
        }
    }

    /// Record that a suggestion was rejected (user typed something else)
    pub fn rejected(&mut self) {
        if let Some(state) = self.pending.take() {
            let decision_time = SystemTime::now()
                .duration_since(state.shown_at)
                .unwrap_or_default()
                .as_millis() as u64;

            self.history.push(Feedback {
                suggestion: state.suggestion,
                context: state.context,
                accepted: false,
                timestamp: SystemTime::now(),
                decision_time_ms: decision_time,
            });
        }
    }

    /// Get feedback history
    pub fn history(&self) -> &[Feedback] {
        &self.history
    }
}

impl Default for FeedbackCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Weight adjuster for graph nodes
pub struct WeightAdjuster;

impl WeightAdjuster {
    /// Adjust weights based on feedback
    pub fn adjust(_graph: &mut ContextGraph, feedback: &[Feedback]) -> Result<(), crate::context::ContextError> {
        for f in feedback {
            if f.accepted {
                // Increase weight of similar commands
                // TODO: Find and update node weights
            } else {
                // Decrease weight
            }
        }
        Ok(())
    }
}

/// Prune old feedback and low-weight nodes
pub fn prune_old_feedback(
    feedback: &mut Vec<Feedback>,
    before: SystemTime,
) -> usize {
    let initial_len = feedback.len();
    feedback.retain(|f| f.timestamp >= before);
    initial_len - feedback.len()
}
