use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, PartialEq)]
pub enum TrackEvent {
    Increment,
    Decrement,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum ViewMode {
    #[default]
    Day,
    Week,
    Month,
    Sparkline,
    Year,
    Stats,
    Heatmap,
}

impl fmt::Display for ViewMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewMode::Day => write!(f, "DAY"),
            ViewMode::Week => write!(f, "WEEK"),
            ViewMode::Month => write!(f, "MONTH"),
            ViewMode::Sparkline => write!(f, "SPARKLINE"),
            ViewMode::Year => write!(f, "YEAR"),
            ViewMode::Stats => write!(f, "STATS"),
            ViewMode::Heatmap => write!(f, "HEATMAP"),
        }
    }
}

/// Whether a habit's `goal` is a per-day target or a per-week (Mon–Sun) one.
/// Persisted on each habit; `#[serde(default)]` means existing records load as
/// `Daily`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GoalPeriod {
    #[default]
    Daily,
    Weekly,
}

impl fmt::Display for GoalPeriod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GoalPeriod::Daily => write!(f, "day"),
            GoalPeriod::Weekly => write!(f, "week"),
        }
    }
}
