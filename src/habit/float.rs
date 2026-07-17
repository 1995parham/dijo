use std::collections::HashMap;
use std::fmt;
use std::ops::{Add, Sub};

use chrono::{Days, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::command::GoalKind;
use crate::habit::traits::Habit;
use crate::habit::{GoalPeriod, InnerData, TrackEvent};
use crate::utils::week_bounds;

#[derive(Copy, Clone, Debug, Ord, Eq, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct FloatData {
    value: u32,
    precision: u8,
}

impl FloatData {
    pub fn add(self, v: u32) -> Self {
        let f = FloatData {
            value: v,
            precision: self.precision,
        };
        self + f
    }
    pub fn sub(self, v: u32) -> Self {
        let f = FloatData {
            value: v,
            precision: self.precision,
        };
        self - f
    }
    pub fn zero() -> Self {
        FloatData {
            value: 0,
            precision: 0,
        }
    }
}

impl fmt::Display for FloatData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let scale = 10u32.pow(self.precision as u32);
        let characteristic = self.value / scale;
        let mantissa = self.value % scale;
        let width = self.precision as usize;
        let s = if self.precision == 0 {
            format!("{characteristic}")
        } else if characteristic == 0 {
            format!(".{mantissa:0width$}")
        } else if mantissa == 0 {
            format!("{characteristic}")
        } else {
            format!("{characteristic}.{mantissa:0width$}")
        };
        write!(f, "{s:^3}")
    }
}

impl Add for FloatData {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            value: self.value + other.value,
            precision: self.precision,
        }
    }
}

impl Sub for FloatData {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            value: self.value.saturating_sub(other.value),
            precision: self.precision,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Float {
    name: String,
    #[serde(default)]
    description: String,
    stats: HashMap<NaiveDate, FloatData>,
    goal: FloatData,
    precision: u8,
    #[serde(default)]
    period: GoalPeriod,

    #[serde(skip)]
    inner_data: InnerData,
}

impl Float {
    pub fn new(name: impl AsRef<str>, goal: u32, precision: u8) -> Self {
        Float {
            name: name.as_ref().to_owned(),
            description: String::new(),
            stats: HashMap::new(),
            goal: FloatData {
                value: goal,
                precision,
            },
            precision,
            period: GoalPeriod::Daily,
            inner_data: Default::default(),
        }
    }

    pub fn with_period(mut self, period: GoalPeriod) -> Self {
        self.period = period;
        self
    }

    /// Sum of every entry's value in the Mon–Sun week containing `date`.
    fn week_total(&self, date: NaiveDate) -> u32 {
        let (monday, sunday) = week_bounds(date);
        let mut day = monday;
        let mut total = 0;
        while day <= sunday {
            total += self.stats.get(&day).map(|v| v.value).unwrap_or(0);
            day = match day.checked_add_days(Days::new(1)) {
                Some(d) => d,
                None => break,
            };
        }
        total
    }
}

impl Habit for Float {
    type HabitType = FloatData;

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
        GoalKind::Float(self.goal.value, self.goal.precision)
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
            GoalPeriod::Weekly => self.week_total(date) >= self.goal.value,
        }
    }
    fn remaining(&self, date: NaiveDate) -> u32 {
        match self.period {
            GoalPeriod::Daily => {
                if self.reached_goal(date) {
                    0
                } else if let Some(&val) = self.stats.get(&date) {
                    (self.goal - val).value
                } else {
                    self.goal.value
                }
            }
            GoalPeriod::Weekly => self.goal.value.saturating_sub(self.week_total(date)),
        }
    }
    fn goal(&self) -> u32 {
        self.goal.value
    }
    fn period(&self) -> GoalPeriod {
        self.period
    }
    fn modify(&mut self, date: NaiveDate, event: TrackEvent) {
        match event {
            TrackEvent::Increment => {
                if let Some(val) = self.stats.get_mut(&date) {
                    *val = val.add(1);
                } else {
                    self.insert_entry(
                        date,
                        FloatData {
                            value: 1,
                            precision: self.precision,
                        },
                    )
                }
            }
            TrackEvent::Decrement => {
                if let Some(val) = self.stats.get_mut(&date) {
                    if *val > FloatData::zero() {
                        *val = val.sub(1);
                    } else {
                        self.stats.remove(&date);
                    };
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
