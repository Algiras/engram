//! Outcome-based learning signals
//!
//! Phase 2: Learn from OUTCOMES, not just usage frequency
//!
//! These signals track whether knowledge actually helped solve problems:
//! - Explicit feedback (helpful/unhelpful)
//! - Error correction (mistake → fix)
//! - First-time success (solved immediately)
//! - Multi-iteration resolution (took multiple attempts)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Explicit user feedback about knowledge quality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplicitFeedback {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub project: String,
    pub knowledge_ids: Vec<String>,
    pub sentiment: Sentiment,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Sentiment {
    Helpful,    // Knowledge solved the problem
    Unhelpful,  // Knowledge didn't help or was wrong
    Neutral,    // Unclear or mixed outcome
}

impl Sentiment {
    pub fn to_reward(&self) -> f32 {
        match self {
            Sentiment::Helpful => 0.8,
            Sentiment::Unhelpful => -0.3,
            Sentiment::Neutral => 0.0,
        }
    }
}

/// Error correction: User provided feedback that knowledge was wrong
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorCorrection {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub project: String,
    pub incorrect_knowledge_id: String,
    pub error_description: String,
    pub correction: Option<String>,
}

impl ErrorCorrection {
    pub fn to_reward(&self) -> f32 {
        -0.5  // Strong negative signal
    }
}

/// Task completed successfully on first attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirstTimeSuccess {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub project: String,
    pub knowledge_ids: Vec<String>,
    pub task_description: String,
}

impl FirstTimeSuccess {
    pub fn to_reward(&self) -> f32 {
        0.9  // Very positive - immediate success
    }
}

/// Task required multiple iterations to solve
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterativeResolution {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub project: String,
    pub knowledge_ids: Vec<String>,
    pub iteration_count: u32,
    pub final_success: bool,
}

impl IterativeResolution {
    pub fn to_reward(&self) -> f32 {
        if !self.final_success {
            return -0.2;  // Failed after multiple attempts
        }

        // Success, but penalize for iterations
        match self.iteration_count {
            1 => 0.9,   // First-time success
            2 => 0.6,   // Needed one retry
            3 => 0.4,   // Needed two retries
            4 => 0.2,   // Needed three retries
            _ => 0.1,   // Too many attempts
        }
    }
}

/// Combined outcome signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutcomeSignal {
    Explicit(ExplicitFeedback),
    ErrorCorrection(ErrorCorrection),
    FirstTimeSuccess(FirstTimeSuccess),
    Iterative(IterativeResolution),
}

impl OutcomeSignal {
    pub fn to_reward(&self) -> f32 {
        match self {
            OutcomeSignal::Explicit(f) => f.sentiment.to_reward(),
            OutcomeSignal::ErrorCorrection(e) => e.to_reward(),
            OutcomeSignal::FirstTimeSuccess(s) => s.to_reward(),
            OutcomeSignal::Iterative(i) => i.to_reward(),
        }
    }

    pub fn knowledge_ids(&self) -> Vec<String> {
        match self {
            OutcomeSignal::Explicit(f) => f.knowledge_ids.clone(),
            OutcomeSignal::ErrorCorrection(e) => vec![e.incorrect_knowledge_id.clone()],
            OutcomeSignal::FirstTimeSuccess(s) => s.knowledge_ids.clone(),
            OutcomeSignal::Iterative(i) => i.knowledge_ids.clone(),
        }
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            OutcomeSignal::Explicit(f) => f.timestamp,
            OutcomeSignal::ErrorCorrection(e) => e.timestamp,
            OutcomeSignal::FirstTimeSuccess(s) => s.timestamp,
            OutcomeSignal::Iterative(i) => i.timestamp,
        }
    }
}

/// Store outcome signals for a project
pub fn save_outcome_signal(memory_dir: &std::path::Path, signal: &OutcomeSignal) -> crate::error::Result<()> {
    let project = match signal {
        OutcomeSignal::Explicit(f) => &f.project,
        OutcomeSignal::ErrorCorrection(e) => &e.project,
        OutcomeSignal::FirstTimeSuccess(s) => &s.project,
        OutcomeSignal::Iterative(i) => &i.project,
    };

    let signals_dir = memory_dir.join("learning").join(project).join("outcome_signals");
    std::fs::create_dir_all(&signals_dir)?;

    let timestamp = signal.timestamp().timestamp();
    let signal_path = signals_dir.join(format!("{}.json", timestamp));

    let json = serde_json::to_string_pretty(signal)?;
    std::fs::write(signal_path, json)?;

    Ok(())
}

/// Load all outcome signals for a project
pub fn load_outcome_signals(memory_dir: &std::path::Path, project: &str) -> crate::error::Result<Vec<OutcomeSignal>> {
    let signals_dir = memory_dir.join("learning").join(project).join("outcome_signals");

    if !signals_dir.exists() {
        return Ok(Vec::new());
    }

    let mut signals = Vec::new();

    for entry in std::fs::read_dir(signals_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            if let Ok(signal) = serde_json::from_str::<OutcomeSignal>(&content) {
                signals.push(signal);
            }
        }
    }

    // Sort by timestamp
    signals.sort_by_key(|s| s.timestamp());

    Ok(signals)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sentiment_rewards() {
        assert_eq!(Sentiment::Helpful.to_reward(), 0.8);
        assert_eq!(Sentiment::Unhelpful.to_reward(), -0.3);
        assert_eq!(Sentiment::Neutral.to_reward(), 0.0);
    }

    #[test]
    fn test_iterative_resolution_rewards() {
        let signal = IterativeResolution {
            timestamp: Utc::now(),
            session_id: "test".to_string(),
            project: "test".to_string(),
            knowledge_ids: vec![],
            iteration_count: 1,
            final_success: true,
        };

        assert_eq!(signal.to_reward(), 0.9);  // First-time success

        let signal2 = IterativeResolution {
            timestamp: Utc::now(),
            session_id: "test".to_string(),
            project: "test".to_string(),
            knowledge_ids: vec![],
            iteration_count: 5,
            final_success: true,
        };

        assert_eq!(signal2.to_reward(), 0.1);  // Too many iterations

        let signal3 = IterativeResolution {
            timestamp: Utc::now(),
            session_id: "test".to_string(),
            project: "test".to_string(),
            knowledge_ids: vec![],
            iteration_count: 3,
            final_success: false,
        };

        assert_eq!(signal3.to_reward(), -0.2);  // Failed
    }

    #[test]
    fn test_error_correction_negative_reward() {
        let signal = ErrorCorrection {
            timestamp: Utc::now(),
            session_id: "test".to_string(),
            project: "test".to_string(),
            incorrect_knowledge_id: "bad-knowledge".to_string(),
            error_description: "This was wrong".to_string(),
            correction: None,
        };

        assert_eq!(signal.to_reward(), -0.5);  // Strong negative
    }
}
