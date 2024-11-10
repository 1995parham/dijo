use std::f64;

use cursive::direction::{Absolute, Direction};
use cursive::event::{Event, EventResult, Key};
use cursive::theme::Color;
use cursive::view::{CannotFocus, View};
use cursive::{Printer, Vec2};

use crate::app::{App, MessageKind};
use crate::habit::ViewMode;
use crate::utils::{GRID_WIDTH, VIEW_HEIGHT, VIEW_WIDTH};

impl View for App {
    fn draw(&self, printer: &Printer) {
        let mut offset = Vec2::zero();
        for (idx, habit) in self.habits.iter().enumerate() {
            if idx >= GRID_WIDTH && idx % GRID_WIDTH == 0 {
                offset = offset.map_y(|y| y + VIEW_HEIGHT).map_x(|_| 0);
            }
            habit.draw(&printer.offset(offset).focused(self.focus == idx));
            offset = offset.map_x(|x| x + VIEW_WIDTH + 2);
        }

        offset = offset.map_x(|_| 0).map_y(|_| self.max_size().y - 2);

        let status = self.status();
        printer.print(offset, &status.0); // left status

        let full = self.max_size().x;
        offset = offset.map_x(|_| full - status.1.len());
        printer.print(offset, &status.1); // right status

        offset = offset.map_x(|_| 0).map_y(|_| self.max_size().y - 1);
        printer.with_style(Color::from(self.message.kind()), |p| {
            p.print(offset, self.message.contents())
        });
    }

    fn required_size(&mut self, _: Vec2) -> Vec2 {
        let width = GRID_WIDTH * (VIEW_WIDTH + 2);
        let height = {
            if !self.habits.is_empty() {
                (VIEW_HEIGHT as f64 * (self.habits.len() as f64 / GRID_WIDTH as f64).ceil())
                    as usize
            } else {
                0
            }
        };
        Vec2::new(width, height + 2)
    }

    fn take_focus(&mut self, _: Direction) -> Result<EventResult, CannotFocus> {
        Err(CannotFocus)
    }

    fn on_event(&mut self, e: Event) -> EventResult {
        if self.habits.is_empty() {
            return EventResult::Ignored;
        }
        match e {
            Event::Key(Key::Right) | Event::Key(Key::Tab) | Event::Char('l') => {
                self.set_focus(Absolute::Right);
                EventResult::Consumed(None)
            }
            Event::Key(Key::Left) | Event::Shift(Key::Tab) | Event::Char('h') => {
                self.set_focus(Absolute::Left);
                EventResult::Consumed(None)
            }
            Event::Key(Key::Up) | Event::Char('k') => {
                self.set_focus(Absolute::Up);
                EventResult::Consumed(None)
            }
            Event::Key(Key::Down) | Event::Char('j') => {
                self.set_focus(Absolute::Down);
                EventResult::Consumed(None)
            }

            Event::Char('K') | Event::Shift(Key::Up) => {
                self.move_cursor(Absolute::Up);
                EventResult::Consumed(None)
            }
            Event::Char('H') | Event::Shift(Key::Left) => {
                self.move_cursor(Absolute::Left);
                EventResult::Consumed(None)
            }
            Event::Char('J') | Event::Shift(Key::Down) => {
                self.move_cursor(Absolute::Down);
                EventResult::Consumed(None)
            }
            Event::Char('L') | Event::Shift(Key::Right) => {
                self.move_cursor(Absolute::Right);
                EventResult::Consumed(None)
            }

            Event::Char('v') => {
                if self.habits.is_empty() {
                    return EventResult::Consumed(None);
                }
                if self.habits[self.focus].inner_data_ref().view_mode() == ViewMode::Week {
                    self.set_mode(ViewMode::Day)
                } else {
                    self.set_mode(ViewMode::Week)
                }
                EventResult::Consumed(None)
            }
            Event::Char('V') => {
                for habit in self.habits.iter_mut() {
                    habit.inner_data_mut_ref().set_view_mode(ViewMode::Week);
                }
                EventResult::Consumed(None)
            }
            Event::Key(Key::Esc) => {
                for habit in self.habits.iter_mut() {
                    habit.inner_data_mut_ref().set_view_mode(ViewMode::Day);
                }
                self.reset_cursor();
                EventResult::Consumed(None)
            }

            /* We want sifting to be an app level function,
             * that later trickles down into each habit
             * */
            Event::Char(']') => {
                self.sift_forward();
                EventResult::Consumed(None)
            }
            Event::Char('[') => {
                self.sift_backward();
                EventResult::Consumed(None)
            }
            Event::Char('}') => {
                self.reset_cursor();
                EventResult::Consumed(None)
            }
            Event::CtrlChar('l') => {
                self.message.clear();
                self.message.set_kind(MessageKind::Info);
                EventResult::Consumed(None)
            }

            /* Every keybind that is not caught by App trickles
             * down to the focused habit.
             * */
            _ => {
                if self.habits.is_empty() {
                    return EventResult::Ignored;
                }
                self.habits[self.focus].on_event(e)
            }
        }
    }
}
