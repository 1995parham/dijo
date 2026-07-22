use cursive::direction::Direction;
use cursive::event::{Event, EventResult, Key};
use cursive::theme::{ColorStyle, Effect, Style};
use cursive::view::{CannotFocus, View};
use cursive::{Printer, Vec2};

use chrono::prelude::*;
use chrono::{Days, Local, NaiveDate};

use crate::habit::{Bit, Count, Float, GoalPeriod, Habit, TrackEvent, ViewMode};
use crate::stats::habit_stats;
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

        // Only the first line of the description sits dimmed under the name,
        // truncated to the cell width; `i` opens the full text in a popup. The
        // Heatmap view already draws on row 1, so it is skipped there.
        if self.inner_data_ref().view_mode() != ViewMode::Heatmap && !self.description().is_empty()
        {
            let first_line = self.description().lines().next().unwrap_or("");
            printer.with_style(future_style, |p| {
                p.print(
                    (0, 1),
                    &format!(" {:.width$} ", first_line, width = VIEW_WIDTH - 4),
                );
            });
        }

        let weekly_goal_habit = self.period() == GoalPeriod::Weekly;
        let draw_week = |printer: &Printer| {
            let days = (1..31)
                .filter_map(|i| NaiveDate::from_ymd_opt(year, month, i)) // dates 28-31 may not exist, ignore them if they don't
                .collect::<Vec<_>>();
            for (week, line_nr) in days.chunks(7).zip(2..) {
                let is_this_week = week.contains(&Local::now().date_naive());
                // For a weekly-goal habit the target lives on the week itself, so
                // read the week's progress off any of its days rather than
                // summing daily targets.
                let (weekly_goal, completions) = if weekly_goal_habit {
                    let goal = self.goal();
                    (goal, goal.saturating_sub(self.remaining(week[0])))
                } else {
                    let goal = self.goal() * week.len() as u32;
                    let remaining = week.iter().map(|&i| self.remaining(i)).sum::<u32>();
                    (goal, goal - remaining)
                };
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

        // A compact one-row month view: each day of the month is a single bar
        // whose height (`▁`..`█`) tracks that day's completion ratio. Reached
        // days are full and coloured as such, partial days sit mid-ramp, missed
        // days show the lowest bar, and future days are left blank.
        let draw_sparkline = |printer: &Printer| {
            const RAMP: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
            let today = Local::now().date_naive();
            let todo_style = Style::from(CONFIGURATION.todo_color());
            let cursor_style = Style::from(ColorStyle::back(cursor_bg()));
            let goal = self.goal().max(1);

            let mut i = 0u32;
            while let Some(d) = NaiveDate::from_ymd_opt(year, month, i + 1) {
                let coords: Vec2 = (i as usize, 2).into();
                let (glyph, mut style): (&str, Style) = if d > today {
                    (" ", future_style)
                } else if self.reached_goal(d) {
                    (RAMP[RAMP.len() - 1], goal_reached_style)
                } else if self.get_by_date(d).is_some() {
                    let done = goal.saturating_sub(self.remaining(d));
                    let idx =
                        ((done as f64 / goal as f64) * (RAMP.len() - 1) as f64).round() as usize;
                    (RAMP[idx.min(RAMP.len() - 1)], todo_style)
                } else {
                    (RAMP[0], future_style)
                };
                if d == now && printer.focused {
                    style = style.combine(cursor_style);
                }
                printer.with_style(style, |p| p.print(coords, glyph));
                i += 1;
            }
        };

        let draw_stats = |printer: &Printer| {
            let today = Local::now().date_naive();

            // every day this habit reached its goal, including months that
            // have since been archived out of the live record
            let reached: Vec<NaiveDate> = self
                .get_dates()
                .into_iter()
                .filter(|&d| self.reached_goal(d))
                .chain(archived.iter().copied())
                .collect();

            let s = habit_stats(&reached, today, self.period());
            let unit = if weekly_goal_habit { "weeks" } else { "days" };

            let lines = [
                format!("Current  {:>4} {unit}", s.current_streak),
                format!("Longest  {:>4} {unit}", s.longest_streak),
                format!(
                    "Done     {:>4} time{}",
                    s.total,
                    if s.total == 1 { "" } else { "s" }
                ),
                format!("Rate     {:>4} %", s.completion_rate),
            ];
            for (i, line) in lines.iter().enumerate() {
                let style = if i == 0 && s.current_streak > 0 {
                    goal_reached_style
                } else {
                    Style::none()
                };
                printer.with_style(style, |p| p.print((2, i + 2), line));
            }
        };

        // GitHub-style contribution grid: 7 weekday rows (Mon..Sun) by a band of
        // trailing weeks. The rightmost column is the week containing the viewed
        // date, so sifting months with [ ] scrolls the heatmap through history.
        let draw_heatmap = |printer: &Printer| {
            let today = Local::now().date_naive();
            let todo_style = Style::from(CONFIGURATION.todo_color());

            const COLS: u64 = VIEW_WIDTH as u64;
            const ROWS: u64 = 7;

            let weekday_off = now.weekday().num_days_from_monday() as u64;
            let anchor_monday = now.checked_sub_days(Days::new(weekday_off)).unwrap_or(now);

            for col in 0..COLS {
                let weeks_back = COLS - 1 - col;
                let week_monday = anchor_monday
                    .checked_sub_days(Days::new(weeks_back * 7))
                    .unwrap_or(anchor_monday);
                for row in 0..ROWS {
                    let date = match week_monday.checked_add_days(Days::new(row)) {
                        Some(d) => d,
                        None => continue,
                    };
                    if date > today {
                        continue; // leave future days blank
                    }
                    let coords: Vec2 = (col as usize, row as usize + 1).into();
                    let (style, glyph): (Style, &str) = if reached_or_archived(date) {
                        (goal_reached_style, "█")
                    } else if self.goal() > 0 && self.remaining(date) < self.goal() {
                        (todo_style, "▒") // some progress, goal not met
                    } else {
                        (future_style, "░") // missed or no data
                    };
                    printer.with_style(style, |p| p.print(coords, glyph));
                }
            }
        };

        match self.inner_data_ref().view_mode() {
            ViewMode::Day => draw_day(printer),
            ViewMode::Week => draw_week(printer),
            ViewMode::Month => draw_month(printer),
            ViewMode::Sparkline => draw_sparkline(printer),
            ViewMode::Year => draw_year(printer),
            ViewMode::Stats => draw_stats(printer),
            ViewMode::Heatmap => draw_heatmap(printer),
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
