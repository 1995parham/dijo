use cursive::direction::Direction;
use cursive::event::{Event, EventResult, Key};
use cursive::theme::{ColorStyle, Effect, Style};
use cursive::view::{CannotFocus, View};
use cursive::{Printer, Vec2};

use std::collections::HashSet;

use chrono::prelude::*;
use chrono::{Local, NaiveDate};

use crate::habit::{Bit, Count, Float, Habit, TrackEvent, ViewMode};
use crate::theme::cursor_bg;
use crate::utils::{VIEW_HEIGHT, VIEW_WIDTH};

use crate::CONFIGURATION;

pub trait ShadowView {
    fn draw(&self, printer: &Printer);
    fn required_size(&mut self, _: Vec2) -> Vec2;
    fn take_focus(&mut self, _: Direction) -> Result<EventResult, CannotFocus>;
    fn on_event(&mut self, e: Event) -> EventResult;
}

// the only way to not rewrite each View implementation for trait
// objects of Habit is to rewrite the View trait itself.
impl<T> ShadowView for T
where
    T: Habit,
    T::HabitType: std::fmt::Display,
{
    fn draw(&self, printer: &Printer) {
        let now = self.inner_data_ref().cursor().0;
        let is_today = now == Local::now().date_naive();
        let year = now.year();
        let month = now.month();

        let goal_reached_style = Style::from(CONFIGURATION.reached_color());
        let future_style = Style::from(CONFIGURATION.inactive_color());

        let strikethrough = Style::from(Effect::Strikethrough);

        let goal_status = is_today && self.reached_goal(Local::now().date_naive());

        printer.with_style(
            Style::merge(&[
                if goal_status {
                    strikethrough
                } else {
                    Style::none()
                },
                if !printer.focused {
                    future_style
                } else {
                    Style::none()
                },
            ]),
            |p| {
                p.print(
                    (0, 0),
                    &format!(" {:.width$} ", self.name(), width = VIEW_WIDTH - 6),
                );
            },
        );

        let draw_week = |printer: &Printer| {
            let days = (1..31)
                .filter_map(|i| NaiveDate::from_ymd_opt(year, month, i)) // dates 28-31 may not exist, ignore them if they don't
                .collect::<Vec<_>>();
            for (week, line_nr) in days.chunks(7).zip(2..) {
                let weekly_goal = self.goal() * week.len() as u32;
                let is_this_week = week.contains(&Local::now().date_naive());
                let remaining = week.iter().map(|&i| self.remaining(i)).sum::<u32>();
                let completions = weekly_goal - remaining;
                let full = VIEW_WIDTH - 8;
                let bars_to_fill = (completions * full as u32)
                    .checked_div(weekly_goal)
                    .unwrap_or(0);
                let percentage = if weekly_goal > 0 {
                    (completions as f64 * 100.) / weekly_goal as f64
                } else {
                    0.0
                };
                printer.with_style(future_style, |p| {
                    p.print((4, line_nr), &"─".repeat(full));
                });
                printer.with_style(goal_reached_style, |p| {
                    p.print((4, line_nr), &"─".repeat(bars_to_fill as usize));
                });
                printer.with_style(
                    if is_this_week {
                        Style::none()
                    } else {
                        future_style
                    },
                    |p| {
                        p.print((0, line_nr), &format!("{percentage:2.0}% "));
                    },
                );
            }
        };

        let draw_day = |printer: &Printer| {
            let mut i = 0;
            while let Some(d) = NaiveDate::from_ymd_opt(year, month, i + 1) {
                let mut day_style = Style::none();
                let mut fs = future_style;
                let grs = ColorStyle::front(CONFIGURATION.reached_color());
                let ts = ColorStyle::front(CONFIGURATION.todo_color());
                let cs = ColorStyle::back(cursor_bg());

                if self.reached_goal(d) {
                    day_style = day_style.combine(Style::from(grs));
                } else {
                    day_style = day_style.combine(Style::from(ts));
                }
                if d == now && printer.focused {
                    day_style = day_style.combine(cs);
                    fs = fs.combine(cs);
                }
                let coords: Vec2 = ((i % 7) * 3, i / 7 + 2).into();
                if let Some(c) = self.get_by_date(d) {
                    printer.with_style(day_style, |p| {
                        p.print(coords, &format!("{c:^3}"));
                    });
                } else if d < now {
                    printer.with_style(fs, |p| {
                        p.print(coords, &format!("{:^3}", CONFIGURATION.look.missing_chr));
                    });
                } else {
                    printer.with_style(fs, |p| {
                        p.print(coords, &format!("{:^3}", CONFIGURATION.look.future_chr));
                    });
                }
                i += 1;
            }
        };

        let archived = &self.inner_data_ref().archived_reached;
        let reached_or_archived =
            |date: NaiveDate| -> bool { self.reached_goal(date) || archived.contains(&date) };

        let draw_month = |printer: &Printer| {
            let today = Local::now().date_naive();
            let months = [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ];
            let todo_style = Style::from(ColorStyle::front(CONFIGURATION.todo_color()));

            for (idx, month_name) in months.iter().enumerate() {
                let month_num = (idx + 1) as u32;
                let col = idx % 3;
                let row = idx / 3;

                let mut total_days = 0u32;
                let mut reached_days = 0u32;
                for day in 1..=31 {
                    if let Some(date) = NaiveDate::from_ymd_opt(year, month_num, day)
                        && date <= today
                    {
                        total_days += 1;
                        if reached_or_archived(date) {
                            reached_days += 1;
                        }
                    }
                }

                let col_width = VIEW_WIDTH / 3;
                let coords: Vec2 = (col * col_width, row + 2).into();

                if total_days == 0 {
                    printer.with_style(future_style, |p| {
                        p.print(coords, &format!("{month_name}  --"));
                    });
                } else {
                    let pct = (reached_days * 100).checked_div(total_days).unwrap_or(0);
                    let style = if reached_days >= total_days {
                        goal_reached_style
                    } else if reached_days > 0 {
                        todo_style
                    } else {
                        future_style
                    };
                    printer.with_style(style, |p| {
                        p.print(coords, &format!("{month_name}{pct:>4}%"));
                    });
                }
            }
        };

        let draw_year = |printer: &Printer| {
            let today = Local::now().date_naive();
            let bar_width = VIEW_WIDTH - 9;

            for i in 0..4 {
                let y = year - 3 + i;
                let line = i as usize + 2;

                let mut total_days = 0u32;
                let mut reached_days = 0u32;
                for m in 1..=12 {
                    for day in 1..=31 {
                        if let Some(date) = NaiveDate::from_ymd_opt(y, m, day)
                            && date <= today
                        {
                            total_days += 1;
                            if reached_or_archived(date) {
                                reached_days += 1;
                            }
                        }
                    }
                }

                let filled = if total_days > 0 {
                    (reached_days as usize * bar_width) / total_days as usize
                } else {
                    0
                };
                let pct = (reached_days * 100).checked_div(total_days).unwrap_or(0);

                printer.with_style(future_style, |p| {
                    p.print((0, line), &format!("{y}"));
                    p.print((4, line), &"─".repeat(bar_width));
                });

                if total_days > 0 {
                    printer.with_style(goal_reached_style, |p| {
                        p.print((4, line), &"─".repeat(filled));
                    });
                    printer.with_style(Style::none(), |p| {
                        p.print((4 + bar_width, line), &format!("{pct:>3}%"));
                    });
                }
            }
        };

        let draw_stats = |printer: &Printer| {
            let today = Local::now().date_naive();

            // every day this habit reached its goal, including months that
            // have since been archived out of the live record
            let mut reached: Vec<NaiveDate> = self
                .get_dates()
                .into_iter()
                .filter(|&d| self.reached_goal(d))
                .chain(archived.iter().copied())
                .collect();
            reached.sort_unstable();
            reached.dedup();

            let total = reached.len() as u32;
            let reached_set: HashSet<NaiveDate> = reached.iter().copied().collect();

            // longest run of consecutive reached days
            let mut longest = 0u32;
            let mut run = 0u32;
            let mut prev: Option<NaiveDate> = None;
            for &d in &reached {
                run = match prev {
                    Some(p) if p.succ_opt() == Some(d) => run + 1,
                    _ => 1,
                };
                longest = longest.max(run);
                prev = Some(d);
            }

            // current streak: count back from today, tolerating a still-open
            // today (start from yesterday if today isn't done yet)
            let mut current = 0u32;
            let mut day = if reached_set.contains(&today) {
                Some(today)
            } else {
                today.pred_opt()
            };
            while let Some(d) = day {
                if reached_set.contains(&d) {
                    current += 1;
                    day = d.pred_opt();
                } else {
                    break;
                }
            }

            // completion rate over the span since the first reached day
            let rate = match reached.first() {
                Some(&first) => {
                    let span = (today - first).num_days() + 1;
                    if span > 0 {
                        (total as i64 * 100 / span) as u32
                    } else {
                        0
                    }
                }
                None => 0,
            };

            let lines = [
                format!("Current  {current:>4} days"),
                format!("Longest  {longest:>4} days"),
                format!(
                    "Done     {total:>4} time{}",
                    if total == 1 { "" } else { "s" }
                ),
                format!("Rate     {rate:>4} %"),
            ];
            for (i, line) in lines.iter().enumerate() {
                let style = if i == 0 && current > 0 {
                    goal_reached_style
                } else {
                    Style::none()
                };
                printer.with_style(style, |p| p.print((2, i + 2), line));
            }
        };

        match self.inner_data_ref().view_mode() {
            ViewMode::Day => draw_day(printer),
            ViewMode::Week => draw_week(printer),
            ViewMode::Month => draw_month(printer),
            ViewMode::Year => draw_year(printer),
            ViewMode::Stats => draw_stats(printer),
        };
    }

    fn required_size(&mut self, _: Vec2) -> Vec2 {
        (VIEW_WIDTH, VIEW_HEIGHT - 2).into()
    }

    fn take_focus(&mut self, _: Direction) -> Result<EventResult, CannotFocus> {
        Ok(EventResult::consumed())
    }

    fn on_event(&mut self, e: Event) -> EventResult {
        let now = self.inner_data_mut_ref().cursor().0;
        match e {
            Event::Key(Key::Enter) | Event::Char('n') | Event::CtrlChar('a') => {
                self.modify(now, TrackEvent::Increment);
                EventResult::Consumed(None)
            }
            Event::Key(Key::Backspace) | Event::Char('p') | Event::CtrlChar('x') => {
                self.modify(now, TrackEvent::Decrement);
                EventResult::Consumed(None)
            }
            _ => EventResult::Ignored,
        }
    }
}

macro_rules! auto_view_impl {
    ($struct_name:ident) => {
        impl View for $struct_name {
            fn draw(&self, printer: &Printer) {
                ShadowView::draw(self, printer);
            }
            fn required_size(&mut self, x: Vec2) -> Vec2 {
                ShadowView::required_size(self, x)
            }
            fn take_focus(&mut self, d: Direction) -> Result<EventResult, CannotFocus> {
                ShadowView::take_focus(self, d)
            }
            fn on_event(&mut self, e: Event) -> EventResult {
                ShadowView::on_event(self, e)
            }
        }
    };
}

macro_rules! generate_view_impls {
    ($($x:ident),*) => (
        $(
            auto_view_impl!($x);
        )*
    );
}

generate_view_impls!(Count, Bit, Float);
