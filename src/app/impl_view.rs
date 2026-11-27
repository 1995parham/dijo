use chrono::{Local, NaiveDate};
use cursive::direction::{Absolute, Direction};
use cursive::event::{Event, EventResult, Key};
use cursive::theme::{BaseColor, Color, ColorStyle, Effect, Style};
use cursive::view::{CannotFocus, View};
use cursive::{Printer, Vec2};

use crate::CONFIGURATION;
use crate::app::{App, MessageKind};
use crate::command::GoalKind;
use crate::habit::{GoalPeriod, HabitWrapper, ViewMode};
use crate::utils::{CARD_GAP, CARD_WIDTH, GRID_WIDTH, VIEW_HEIGHT, VIEW_WIDTH};

/// The progress badge in the top-right corner of a card: a check mark once the
/// goal of today (or of this week) is reached, the percentage until then.
fn badge(habit: &dyn HabitWrapper, accent: Color, today: NaiveDate) -> Option<(String, Style)> {
    if let GoalKind::Limit(limit) = habit.kind() {
        return Some(if habit.remaining(today) == 0 {
            (
                format!("✓ ≤{limit}"),
                Style::from(accent).combine(Effect::Bold),
            )
        } else if habit.has_entry(today) {
            (
                "over".to_string(),
                Style::from(CONFIGURATION.undone_color()),
            )
        } else {
            (
                format!("≤{limit}"),
                Style::from(CONFIGURATION.inactive_color()),
            )
        });
    }
    let goal = habit.goal();
    if goal == 0 {
        return None;
    }
    let done = goal.saturating_sub(habit.remaining(today));
    let (text, style) = if done >= goal {
        ("✓".to_string(), Style::from(accent).combine(Effect::Bold))
    } else if done > 0 {
        let pct = u64::from(done) * 100 / u64::from(goal);
        (format!("{pct}%"), Style::from(CONFIGURATION.todo_color()))
    } else {
        (
            "0%".to_string(),
            Style::from(CONFIGURATION.inactive_color()),
        )
    };
    let text = match habit.period() {
        GoalPeriod::Weekly => format!("week {text}"),
        GoalPeriod::Daily => text,
    };
    Some((text, style))
}

/// Draw one habit as a card: a rounded frame in the habit's color when it has
/// the focus, the name and the progress badge on the top border, and the
/// habit's own view inside.
fn draw_card(
    printer: &Printer,
    habit: &dyn HabitWrapper,
    accent: Color,
    focused: bool,
    today: NaiveDate,
) {
    let border = if focused {
        accent
    } else {
        CONFIGURATION.inactive_color()
    };
    let line = "─".repeat(CARD_WIDTH - 2);
    printer.with_style(border, |p| {
        p.print((0, 0), &format!("╭{line}╮"));
        for y in 1..VIEW_HEIGHT - 1 {
            p.print((0, y), "│");
            p.print((CARD_WIDTH - 1, y), "│");
        }
        p.print((0, VIEW_HEIGHT - 1), &format!("╰{line}╯"));
    });

    let badge = badge(habit, accent, today);
    let badge_width = badge
        .as_ref()
        .map_or(0, |(text, _)| text.chars().count() + 3);
    let name: String = habit
        .name()
        .chars()
        .take(CARD_WIDTH - 6 - badge_width)
        .collect();
    let title = if focused {
        Style::from(accent).combine(Effect::Bold)
    } else {
        Style::none()
    };
    printer.with_style(title, |p| p.print((2, 0), &format!(" {name} ")));
    if let Some((text, style)) = badge {
        let x = CARD_WIDTH - 4 - text.chars().count();
        printer.with_style(style, |p| p.print((x, 0), &format!(" {text} ")));
    }

    let body = printer
        .offset((1, 1))
        .cropped((VIEW_WIDTH, VIEW_HEIGHT - 2))
        .focused(focused);
    habit.draw(&body, accent);
}

impl View for App {
    fn draw(&self, printer: &Printer) {
        let today = Local::now().date_naive();
        let dim = CONFIGURATION.inactive_color();

        if self.habits.is_empty() {
            printer.print((2, 1), "No habits yet.");
            printer.with_style(dim, |p| {
                p.print((2, 2), "Press : and type `add <name> [goal]` to add one.");
            });
        }
        for (idx, habit) in self.habits.iter().enumerate() {
            let origin = Vec2::new(
                (idx % self.cols) * (CARD_WIDTH + CARD_GAP),
                (idx / self.cols) * VIEW_HEIGHT,
            );
            draw_card(
                &printer.offset(origin),
                habit.as_ref(),
                CONFIGURATION.accent(idx),
                idx == self.focus,
                today,
            );
        }

        // status line: the view mode as a badge in the color of the focused
        // habit, how many habits are done today, and the date under the cursor
        let y = self.grid_height();
        let status = self.status();
        let mode = format!(" {} ", status.mode);
        let mode_style = ColorStyle::new(
            Color::Dark(BaseColor::Black),
            CONFIGURATION.accent(self.focus),
        );
        printer.with_style(Style::from(mode_style).combine(Effect::Bold), |p| {
            p.print((0, y), &mode);
        });
        let mut x = mode.chars().count() + 1;
        const BAR: usize = 10;
        if let Some(filled) = (status.done * BAR).checked_div(status.total) {
            printer.with_style(dim, |p| p.print((x, y), &"━".repeat(BAR)));
            printer.with_style(CONFIGURATION.accent(self.focus), |p| {
                p.print((x, y), &"━".repeat(filled));
            });
            x += BAR + 1;
            let text = format!("{}/{} done today", status.done, status.total);
            printer.print((x, y), &text);
            x += text.chars().count() + 1;
        }
        let stamp = status.timestamp.chars().count();
        if x + stamp <= self.frame_width() {
            printer.with_style(dim, |p| {
                p.print((self.frame_width() - stamp, y), &status.timestamp);
            });
        }

        printer.with_style(Color::from(self.message.kind()), |p| {
            p.print((0, y + 1), self.message.contents())
        });
    }

    fn required_size(&mut self, constraint: Vec2) -> Vec2 {
        // as many cards in a row as the terminal has room for
        self.cols = ((constraint.x + CARD_GAP) / (CARD_WIDTH + CARD_GAP)).clamp(1, GRID_WIDTH);
        Vec2::new(self.frame_width(), self.grid_height() + 2)
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
                let next = match self.habits[self.focus].inner_data_ref().view_mode() {
                    ViewMode::Day => ViewMode::Week,
                    ViewMode::Week => ViewMode::Month,
                    ViewMode::Month => ViewMode::Sparkline,
                    ViewMode::Sparkline => ViewMode::Year,
                    ViewMode::Year => ViewMode::Stats,
                    ViewMode::Stats => ViewMode::Heatmap,
                    ViewMode::Heatmap => ViewMode::Day,
                };
                for habit in self.habits.iter_mut() {
                    habit.inner_data_mut_ref().set_view_mode(next);
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

            // open the focused habit's full-screen dashboard; the actual layer
            // push needs the Cursive root, so defer it to a callback
            Event::Char('d') => EventResult::with_cb(crate::command::open_dashboard),

            // open the focused habit's description in a popup; the layer push
            // needs the Cursive root, so defer it to a callback
            Event::Char('i') => EventResult::with_cb(crate::command::open_description),

            /* We want sifting to be an app level function,
             * that later trickles down into each habit
             * */
            Event::Char('<') => {
                self.move_focused(Absolute::Left);
                EventResult::Consumed(None)
            }
            Event::Char('>') => {
                self.move_focused(Absolute::Right);
                EventResult::Consumed(None)
            }
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
            _ => self.habits[self.focus].on_event(e),
        }
    }
}
