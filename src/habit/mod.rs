mod traits;
pub use traits::{Habit, HabitWrapper};

mod count;
pub use count::Count;

mod bit;
pub use bit::Bit;

mod float;
pub use float::Float;

mod prelude;
pub use prelude::{TrackEvent, ViewMode};

use crate::app::Cursor;

use chrono::NaiveDate;
use cursive::direction::Absolute;
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct InnerData {
    pub cursor: Cursor,
    pub view_mode: ViewMode,
    pub archived_reached: HashSet<NaiveDate>,
}

impl InnerData {
    pub fn move_cursor(&mut self, d: Absolute) {
        self.cursor.small_seek(d);
    }
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
    }
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }
    pub fn archived_reached_goal(&self, date: NaiveDate) -> bool {
        self.archived_reached.contains(&date)
    }
}
