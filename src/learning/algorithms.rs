use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Temporal Difference learning for knowledge importance
pub fn learn_importance(current: f32, reward: f32, learning_rate: f32) -> f32 {
    // Clamp to valid range [0.1, 1.0]
    (current + learning_rate * (reward - current))
        .max(0.1)
        .min(1.0)
}

/// State representation for Q-learning
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TTLState {
    pub importance_tier: ImportanceTier,
    pub usage_frequency_tier: FrequencyTier,
    pub recency_tier: RecencyTier,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ImportanceTier {
    Low,    // 0.0-0.4
    Medium, // 0.4-0.7
    High,   // 0.7-1.0
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FrequencyTier {
    Rare,      // 0-5 accesses
    Occasional, // 5-20 accesses
    Frequent,  // 20+ accesses
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecencyTier {
    Stale,  // >30 days
    Normal, // 7-30 days
    Recent, // <7 days
}

/// Actions for TTL adjustment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TTLAction {
    Extend7d,
    Extend14d,
    Extend30d,
    Extend90d,
    MakePermanent,
    Reduce1d,
    Reduce3d,
    Reduce7d,
}

impl TTLAction {
    pub fn apply_to_days(&self, current_ttl_days: Option<u32>) -> Option<u32> {
        match self {
            TTLAction::Extend7d => Some(current_ttl_days.unwrap_or(7) + 7),
            TTLAction::Extend14d => Some(current_ttl_days.unwrap_or(14) + 14),
            TTLAction::Extend30d => Some(current_ttl_days.unwrap_or(30) + 30),
            TTLAction::Extend90d => Some(current_ttl_days.unwrap_or(90) + 90),
            TTLAction::MakePermanent => None, // No TTL = permanent
            TTLAction::Reduce1d => Some(current_ttl_days.unwrap_or(7).saturating_sub(1).max(1)),
            TTLAction::Reduce3d => Some(current_ttl_days.unwrap_or(7).saturating_sub(3).max(1)),
            TTLAction::Reduce7d => Some(current_ttl_days.unwrap_or(7).saturating_sub(7).max(1)),
        }
    }
}

/// Q-Learning for TTL policy optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TTLQLearning {
    pub q_table: HashMap<(TTLState, TTLAction), f32>,
    pub learning_rate: f32,
    pub discount_factor: f32,
    pub epsilon: f32, // Exploration rate
}

impl Default for TTLQLearning {
    fn default() -> Self {
        Self {
            q_table: HashMap::new(),
            learning_rate: 0.1,
            discount_factor: 0.9,
            epsilon: 0.2,
        }
    }
}

impl TTLQLearning {
    /// Choose action using epsilon-greedy strategy
    pub fn choose_action(&self, state: &TTLState) -> TTLAction {
        use rand::Rng;

        // Explore with probability epsilon
        if rand::thread_rng().gen::<f32>() < self.epsilon {
            return self.random_action();
        }

        // Exploit: choose best known action
        self.best_action(state)
    }

    /// Get the best known action for a state
    fn best_action(&self, state: &TTLState) -> TTLAction {
        let all_actions = vec![
            TTLAction::Extend7d,
            TTLAction::Extend14d,
            TTLAction::Extend30d,
            TTLAction::Extend90d,
            TTLAction::MakePermanent,
            TTLAction::Reduce1d,
            TTLAction::Reduce3d,
            TTLAction::Reduce7d,
        ];

        all_actions
            .into_iter()
            .max_by(|a, b| {
                let q_a = self.get_q_value(state, a);
                let q_b = self.get_q_value(state, b);
                q_a.partial_cmp(&q_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(TTLAction::Extend7d)
    }

    /// Get Q-value for state-action pair
    fn get_q_value(&self, state: &TTLState, action: &TTLAction) -> f32 {
        *self
            .q_table
            .get(&(state.clone(), *action))
            .unwrap_or(&0.0)
    }

    /// Random action for exploration
    fn random_action(&self) -> TTLAction {
        use rand::seq::SliceRandom;
        let actions = vec![
            TTLAction::Extend7d,
            TTLAction::Extend14d,
            TTLAction::Extend30d,
            TTLAction::Extend90d,
            TTLAction::MakePermanent,
            TTLAction::Reduce1d,
            TTLAction::Reduce3d,
            TTLAction::Reduce7d,
        ];
        *actions.choose(&mut rand::thread_rng()).unwrap()
    }

    /// Update Q-table based on observed reward
    pub fn update(
        &mut self,
        state: TTLState,
        action: TTLAction,
        reward: f32,
        next_state: TTLState,
    ) {
        let current_q = self.get_q_value(&state, &action);
        let max_next_q = self.best_q_value(&next_state);

        let new_q =
            current_q + self.learning_rate * (reward + self.discount_factor * max_next_q - current_q);

        self.q_table.insert((state, action), new_q);
    }

    /// Get the maximum Q-value for a state (over all actions)
    fn best_q_value(&self, state: &TTLState) -> f32 {
        let all_actions = vec![
            TTLAction::Extend7d,
            TTLAction::Extend14d,
            TTLAction::Extend30d,
            TTLAction::Extend90d,
            TTLAction::MakePermanent,
            TTLAction::Reduce1d,
            TTLAction::Reduce3d,
            TTLAction::Reduce7d,
        ];

        all_actions
            .iter()
            .map(|a| self.get_q_value(state, a))
            .fold(0.0, f32::max)
    }
}

/// Consolidation strategy for multi-armed bandit
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ConsolidationStrategy {
    pub similarity_threshold: f32,
    pub trigger_frequency_days: u32,
    pub size_trigger_mb: f32,
}

/// Multi-armed bandit for consolidation strategy selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationBandit {
    pub arms: Vec<ConsolidationStrategy>,
    pub rewards: Vec<Vec<f32>>, // History of rewards for each arm
    pub epsilon: f32,
}

impl Default for ConsolidationBandit {
    fn default() -> Self {
        let arms = vec![
            ConsolidationStrategy {
                similarity_threshold: 0.85,
                trigger_frequency_days: 7,
                size_trigger_mb: 5.0,
            },
            ConsolidationStrategy {
                similarity_threshold: 0.90,
                trigger_frequency_days: 7,
                size_trigger_mb: 10.0,
            },
            ConsolidationStrategy {
                similarity_threshold: 0.90,
                trigger_frequency_days: 14,
                size_trigger_mb: 10.0,
            },
            ConsolidationStrategy {
                similarity_threshold: 0.95,
                trigger_frequency_days: 30,
                size_trigger_mb: 15.0,
            },
        ];

        Self {
            rewards: vec![Vec::new(); arms.len()],
            arms,
            epsilon: 0.2,
        }
    }
}

impl ConsolidationBandit {
    /// Select an arm using epsilon-greedy
    pub fn select_arm(&self) -> usize {
        use rand::Rng;

        // Explore with probability epsilon
        if rand::thread_rng().gen::<f32>() < self.epsilon {
            return rand::thread_rng().gen_range(0..self.arms.len());
        }

        // Exploit: choose arm with highest average reward
        self.best_arm()
    }

    /// Get the arm with the highest average reward
    fn best_arm(&self) -> usize {
        self.rewards
            .iter()
            .enumerate()
            .map(|(i, rewards)| {
                let avg = if rewards.is_empty() {
                    0.0
                } else {
                    rewards.iter().sum::<f32>() / rewards.len() as f32
                };
                (i, avg)
            })
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Update rewards after observing outcome
    pub fn update_reward(&mut self, arm: usize, reward: f32) {
        if arm < self.rewards.len() {
            self.rewards[arm].push(reward);
        }
    }

    /// Get the best strategy based on learned rewards
    pub fn best_strategy(&self) -> ConsolidationStrategy {
        self.arms[self.best_arm()]
    }
}

/// Learn consolidation parameters from usage
pub fn learn_consolidation(
    bandit: &mut ConsolidationBandit,
    health_improvement: f32,
    query_perf_improvement: f32,
    user_acceptance_rate: f32,
) -> ConsolidationStrategy {
    // Compute reward as weighted combination
    let reward = 0.4 * health_improvement + 0.3 * query_perf_improvement + 0.3 * user_acceptance_rate;

    let last_arm = bandit.select_arm();
    bandit.update_reward(last_arm, reward);

    bandit.best_strategy()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learn_importance() {
        let current = 0.5;
        let reward = 0.8;
        let learning_rate = 0.1;

        let new_importance = learn_importance(current, reward, learning_rate);
        assert_eq!(new_importance, 0.53);

        // Test clamping
        assert!(learn_importance(0.0, 0.0, 0.5) >= 0.1);
        assert!(learn_importance(1.5, 1.5, 0.5) <= 1.0);
    }

    #[test]
    fn test_ttl_action_apply() {
        assert_eq!(TTLAction::Extend7d.apply_to_days(Some(7)), Some(14));
        assert_eq!(TTLAction::MakePermanent.apply_to_days(Some(30)), None);
        assert_eq!(TTLAction::Reduce3d.apply_to_days(Some(7)), Some(4));
        assert_eq!(TTLAction::Reduce7d.apply_to_days(Some(3)), Some(1)); // Min 1 day
    }

    #[test]
    fn test_q_learning_update() {
        let mut q = TTLQLearning::default();

        let state = TTLState {
            importance_tier: ImportanceTier::High,
            usage_frequency_tier: FrequencyTier::Frequent,
            recency_tier: RecencyTier::Recent,
        };

        let action = TTLAction::Extend30d;
        let reward = 0.8;

        let next_state = state.clone();

        q.update(state.clone(), action, reward, next_state);

        let q_value = q.get_q_value(&state, &action);
        assert!(q_value > 0.0);
    }

    #[test]
    fn test_consolidation_bandit() {
        let mut bandit = ConsolidationBandit::default();

        // Simulate rewards
        bandit.update_reward(0, 0.5);
        bandit.update_reward(1, 0.8);
        bandit.update_reward(1, 0.9);

        // Arm 1 should have higher average
        let best = bandit.best_arm();
        assert_eq!(best, 1);
    }
}
