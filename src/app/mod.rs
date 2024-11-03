use std::default::Default;

use crate::habit::HabitWrapper;

mod cursor;
mod impl_self;
mod impl_view;
mod message;

pub struct StatusLine(String, String);
pub use cursor::Cursor;
pub use message::{Message, MessageKind};

pub struct App {
    // holds app data (habit_record.json)
    habits: Vec<Box<dyn HabitWrapper>>,

    focus: usize,
    cursor: Cursor,
    message: Message,
}

impl Default for App {
    fn default() -> Self {
        App::new()
    }
}
