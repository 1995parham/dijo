use std::collections::HashMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::command::GoalKind;
use crate::habit::traits::Habit;
use crate::habit::{GoalPeriod, InnerData, TrackEvent};

/// A habit to keep under a daily limit, such as `:add smoke <5`. A day is
/// reached when it has an entry at or below the limit; a day over the limit is
/// undone. A day needs an entry to count, so `p` on an empty day records a
/// clean day (0).
#[derive(Debug, Serialize, Deserialize)]
pub struct Limit {
    name: String,
    #[serde(default)]
    description: String,
    stats: HashMap<NaiveDate, u32>,
    /// The most that a day may have.
    goal: u32,

    #[serde(skip)]
    inner_data: InnerData,
}

impl Limit {
    pub fn new(name: impl AsRef<str>, limit: u32) -> Self {
        Limit {
            name: name.as_ref().to_owned(),
            description: String::new(),
            stats: HashMap::new(),
            goal: limit,
            inner_data: Default::default(),
        }
    }
}

impl Habit for Limit {
    type HabitType = u32;

    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        &self.description
    }
    fn set_description(&mut self, description: String) {
        self.description = description;
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn set_goal(&mut self, goal: &GoalKind, period: GoalPeriod) -> Result<(), String> {
        match (goal, period) {
            (GoalKind::Limit(n), GoalPeriod::Daily) => {
                self.goal = *n;
                Ok(())
            }
            _ => Err("a limit habit takes a daily limit such as `<5`".into()),
        }
    }
    fn kind(&self) -> GoalKind {
        GoalKind::Limit(self.goal)
    }
    fn get_by_date(&self, date: NaiveDate) -> Option<&Self::HabitType> {
        self.stats.get(&date)
    }
    fn get_dates(&self) -> Vec<NaiveDate> {
        self.stats.keys().copied().collect()
    }
    fn insert_entry(&mut self, date: NaiveDate, val: Self::HabitType) {
        self.stats.insert(date, val);
    }
    fn reached_goal(&self, date: NaiveDate) -> bool {
        self.stats.get(&date).is_some_and(|&val| val <= self.goal)
    }
    /// Nothing when the day is in the limit, the whole goal otherwise, so the
    /// views draw a day as reached or not, with no partial shades.
    fn remaining(&self, date: NaiveDate) -> u32 {
        if self.reached_goal(date) {
            0
        } else {
            self.goal.max(1)
        }
    }
    /// The limit; at least 1, so the views see a goal even for `<0`.
    fn goal(&self) -> u32 {
        self.goal.max(1)
    }
    fn modify(&mut self, date: NaiveDate, event: TrackEvent) {
        match event {
            TrackEvent::Increment => *self.stats.entry(date).or_insert(0) += 1,
            TrackEvent::Decrement => match self.stats.get_mut(&date) {
                Some(0) => {
                    self.stats.remove(&date);
                }
                Some(val) => *val -= 1,
                // a clean day
                None => self.insert_entry(date, 0),
            },
        }
    }
    fn inner_data_ref(&self) -> &InnerData {
        &self.inner_data
    }
    fn inner_data_mut_ref(&mut self) -> &mut InnerData {
        &mut self.inner_data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, 1, day).unwrap()
    }

    #[test]
    fn a_day_in_the_limit_is_reached() {
        let mut h = Limit::new("smoke", 2);
        h.insert_entry(d(1), 2);
        h.insert_entry(d(2), 3);
        assert!(h.reached_goal(d(1)));
        assert!(!h.reached_goal(d(2)), "over the limit");
        assert!(!h.reached_goal(d(3)), "no entry is not filled");
        assert_eq!(h.remaining(d(1)), 0);
        assert_eq!(h.remaining(d(2)), 2);
    }

    #[test]
    fn decrement_on_an_empty_day_records_a_clean_day() {
        let mut h = Limit::new("smoke", 0);
        h.modify(d(1), TrackEvent::Decrement);
        assert_eq!(h.get_by_date(d(1)), Some(&0));
        assert!(h.reached_goal(d(1)));

        h.modify(d(1), TrackEvent::Increment);
        assert!(!h.reached_goal(d(1)), "one over a limit of zero");

        h.modify(d(1), TrackEvent::Decrement);
        h.modify(d(1), TrackEvent::Decrement);
        assert_eq!(h.get_by_date(d(1)), None, "decrement at 0 clears the day");
    }
}
