use std::collections::HashMap;
use std::default::Default;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::CONFIGURATION;
use crate::command::GoalKind;
use crate::habit::traits::Habit;
use crate::habit::{InnerData, TrackEvent};

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct CustomBool(bool);

use std::fmt;
impl fmt::Display for CustomBool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:^3}",
            if self.0 {
                CONFIGURATION.look.true_chr
            } else {
                CONFIGURATION.look.false_chr
            }
        )
    }
}

impl From<bool> for CustomBool {
    fn from(b: bool) -> Self {
        CustomBool(b)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Bit {
    name: String,
    stats: HashMap<NaiveDate, CustomBool>,
    goal: CustomBool,

    #[serde(skip)]
    inner_data: InnerData,
}

impl Bit {
    pub fn new(name: impl AsRef<str>) -> Self {
        Bit {
            name: name.as_ref().to_owned(),
            stats: HashMap::new(),
            goal: CustomBool(true),
            inner_data: Default::default(),
        }
    }
}

impl Habit for Bit {
    type HabitType = CustomBool;
    fn name(&self) -> String {
        self.name.clone()
    }
    fn kind(&self) -> GoalKind {
        GoalKind::Bit
    }
    fn get_by_date(&self, date: NaiveDate) -> Option<&Self::HabitType> {
        self.stats.get(&date)
    }
    fn insert_entry(&mut self, date: NaiveDate, val: Self::HabitType) {
        *self.stats.entry(date).or_insert(val) = val;
    }
    fn reached_goal(&self, date: NaiveDate) -> bool {
        if let Some(val) = self.stats.get(&date)
            && val.0 >= self.goal.0
        {
            return true;
        }
        false
    }
    fn remaining(&self, date: NaiveDate) -> u32 {
        if let Some(val) = self.stats.get(&date) {
            if val.0 { 0 } else { 1 }
        } else {
            1
        }
    }
    fn goal(&self) -> u32 {
        1
    }
    fn modify(&mut self, date: NaiveDate, event: TrackEvent) {
        match event {
            TrackEvent::Increment => {
                if let Some(val) = self.stats.get_mut(&date) {
                    *val = (val.0 ^ true).into()
                } else {
                    self.insert_entry(date, CustomBool(true));
                }
            }
            TrackEvent::Decrement => {
                if let Some(val) = self.stats.get_mut(&date) {
                    if val.0 {
                        *val = false.into();
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
