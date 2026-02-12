pub mod tracker;
pub mod metrics;
pub mod insights;

pub use tracker::{EventTracker, UsageEvent, EventType};
pub use insights::generate_insights;
