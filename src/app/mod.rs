use std::default::Default;

use crate::habit::{HabitWrapper, ViewMode};

mod cursor;
mod impl_self;
mod impl_view;
mod message;

/// What the status line shows.
pub struct StatusLine {
    mode: ViewMode,
    /// Habits whose goal for today (or this week) is reached.
    done: usize,
    /// Habits that have a goal.
    total: usize,
    timestamp: String,
}
pub use cursor::Cursor;
pub use message::{Message, MessageKind};

pub struct App {
    // holds app data (habit_record.json)
    habits: Vec<Box<dyn HabitWrapper>>,

    focus: usize,
    /// cards in one grid row, set from the terminal width on each layout
    cols: usize,
    cursor: Cursor,
    message: Message,
}

impl Default for App {
    fn default() -> Self {
        App::new()
    }
}
