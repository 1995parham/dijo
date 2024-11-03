use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, PartialEq)]
pub enum TrackEvent {
    Increment,
    Decrement,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[derive(Default)]
pub enum ViewMode {
    #[default]
    Day,
    Week,
    Month,
    Year,
}


impl fmt::Display for ViewMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewMode::Day => write!(f, "DAY"),
            ViewMode::Week => write!(f, "WEEK"),
            ViewMode::Month => write!(f, "MONTH"),
            ViewMode::Year => write!(f, "YEAR"),
        }
    }
}

pub fn default_auto() -> bool {
    false
}
