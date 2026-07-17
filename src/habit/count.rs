use std::collections::HashMap;

use chrono::{Days, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::command::GoalKind;
use crate::habit::traits::Habit;
use crate::habit::{GoalPeriod, InnerData, TrackEvent};
use crate::utils::week_bounds;

#[derive(Debug, Serialize, Deserialize)]
pub struct Count {
    name: String,
    #[serde(default)]
    description: String,
    stats: HashMap<NaiveDate, u32>,
    goal: u32,
    #[serde(default)]
    period: GoalPeriod,

    #[serde(skip)]
    inner_data: InnerData,
}

impl Count {
    pub fn new(name: impl AsRef<str>, goal: u32) -> Self {
        Count {
            name: name.as_ref().to_owned(),
            description: String::new(),
            stats: HashMap::new(),
            goal,
            period: GoalPeriod::Daily,
            inner_data: Default::default(),
        }
    }

    pub fn with_period(mut self, period: GoalPeriod) -> Self {
        self.period = period;
        self
    }

    /// Sum of every entry in the Mon–Sun week containing `date`.
    fn week_total(&self, date: NaiveDate) -> u32 {
        let (monday, sunday) = week_bounds(date);
        let mut day = monday;
        let mut total = 0;
        while day <= sunday {
            total += self.stats.get(&day).copied().unwrap_or(0);
            day = match day.checked_add_days(Days::new(1)) {
                Some(d) => d,
                None => break,
            };
        }
        total
    }
}

impl Habit for Count {
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
    fn kind(&self) -> GoalKind {
        GoalKind::Count(self.goal)
    }
    fn get_by_date(&self, date: NaiveDate) -> Option<&Self::HabitType> {
        self.stats.get(&date)
    }
    fn get_dates(&self) -> Vec<NaiveDate> {
        self.stats.keys().copied().collect()
    }
    fn insert_entry(&mut self, date: NaiveDate, val: Self::HabitType) {
        *self.stats.entry(date).or_insert(val) = val;
    }
    fn reached_goal(&self, date: NaiveDate) -> bool {
        match self.period {
            GoalPeriod::Daily => self.stats.get(&date).is_some_and(|val| val >= &self.goal),
            GoalPeriod::Weekly => self.week_total(date) >= self.goal,
        }
    }
    fn remaining(&self, date: NaiveDate) -> u32 {
        match self.period {
            GoalPeriod::Daily => {
                if self.reached_goal(date) {
                    0
                } else if let Some(val) = self.stats.get(&date) {
                    self.goal - val
                } else {
                    self.goal
                }
            }
            GoalPeriod::Weekly => self.goal.saturating_sub(self.week_total(date)),
        }
    }
    fn goal(&self) -> u32 {
        self.goal
    }
    fn period(&self) -> GoalPeriod {
        self.period
    }
    fn modify(&mut self, date: NaiveDate, event: TrackEvent) {
        match event {
            TrackEvent::Increment => {
                if let Some(val) = self.stats.get_mut(&date) {
                    *val += 1
                } else {
                    self.insert_entry(date, 1);
                }
            }
            TrackEvent::Decrement => {
                if let Some(val) = self.stats.get_mut(&date) {
                    if *val > 0 {
                        *val -= 1
                    } else {
                        self.stats.remove(&date);
                    }
                }
            }
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

    // 2024-01-01 is a Monday, so 01..=07 Jan is one Mon–Sun week.
    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, 1, day).unwrap()
    }

    #[test]
    fn weekly_goal_aggregates_the_whole_week() {
        let mut h = Count::new("gym", 3).with_period(GoalPeriod::Weekly);
        h.insert_entry(d(1), 1); // Mon
        h.insert_entry(d(3), 1); // Wed
        assert!(!h.reached_goal(d(1)), "2 of 3 done, week not reached yet");
        assert_eq!(h.remaining(d(5)), 1, "one more needed anywhere in the week");

        h.insert_entry(d(5), 1); // Fri -> hits the weekly target
        // Every day of the reached week now reports reached / nothing remaining.
        for day in 1..=7 {
            assert!(h.reached_goal(d(day)));
            assert_eq!(h.remaining(d(day)), 0);
        }
    }

    #[test]
    fn weekly_goal_does_not_bleed_across_weeks() {
        let mut h = Count::new("gym", 2).with_period(GoalPeriod::Weekly);
        h.insert_entry(d(1), 2); // fills the first week only
        assert!(h.reached_goal(d(2)));
        // The following Mon–Sun week (08..=14 Jan) is untouched.
        assert!(!h.reached_goal(d(8)));
        assert_eq!(h.remaining(d(10)), 2);
    }

    #[test]
    fn period_survives_a_serde_round_trip() {
        let h = Count::new("gym", 3).with_period(GoalPeriod::Weekly);
        let json = serde_json::to_string(&h).unwrap();
        assert!(json.contains("\"period\":\"Weekly\""), "json was {json}");
        let back: Count = serde_json::from_str(&json).unwrap();
        assert_eq!(back.period, GoalPeriod::Weekly);
    }

    #[test]
    fn legacy_records_without_a_period_load_as_daily() {
        // A record written before weekly goals existed has no `period` key.
        let json = r#"{"name":"read","description":"","stats":{},"goal":2}"#;
        let h: Count = serde_json::from_str(json).unwrap();
        assert_eq!(h.period, GoalPeriod::Daily);
    }

    #[test]
    fn daily_goal_is_unchanged_by_the_period_field() {
        let mut h = Count::new("water", 3); // defaults to daily
        h.insert_entry(d(1), 3);
        assert!(h.reached_goal(d(1)));
        assert!(!h.reached_goal(d(2)));
        assert_eq!(h.remaining(d(2)), 3);
    }
}
