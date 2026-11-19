use cursive::direction::Direction;
use cursive::event::{Event, EventResult, Key};
use cursive::theme::{Color, Effect, Style};
use cursive::view::CannotFocus;
use cursive::{Printer, Vec2};

use chrono::prelude::*;
use chrono::{Days, Local, NaiveDate};

use crate::habit::{GoalPeriod, Habit, TrackEvent, ViewMode};
use crate::stats::habit_stats;
use crate::theme::progress_color;
use crate::utils::{VIEW_HEIGHT, VIEW_WIDTH, week_bounds};

use crate::CONFIGURATION;

/// Bars of rising height, for the sparkline and the month view.
const RAMP: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

pub trait ShadowView {
    /// Draw the body of a habit card. The frame and the title belong to the
    /// grid; `accent` is the color the grid gave this habit.
    fn draw(&self, printer: &Printer, accent: Color);
    fn required_size(&mut self, _: Vec2) -> Vec2;
    fn take_focus(&mut self, _: Direction) -> Result<EventResult, CannotFocus>;
    fn on_event(&mut self, e: Event) -> EventResult;
}

/// A horizontal bar of `width` cells with the first `filled` in `color`.
fn draw_bar(printer: &Printer, at: (usize, usize), width: usize, filled: usize, color: Color) {
    printer.with_style(CONFIGURATION.inactive_color(), |p| {
        p.print(at, &"━".repeat(width));
    });
    printer.with_style(color, |p| {
        p.print(at, &"━".repeat(filled.min(width)));
    });
}

fn ramp(ratio: f64) -> &'static str {
    let idx = (ratio.clamp(0.0, 1.0) * (RAMP.len() - 1) as f64).round() as usize;
    RAMP[idx.min(RAMP.len() - 1)]
}

// the only way to not rewrite each View implementation for trait
// objects of Habit is to rewrite the View trait itself.
impl<T> ShadowView for T
where
    T: Habit,
    T::HabitType: std::fmt::Display,
{
    fn draw(&self, printer: &Printer, accent: Color) {
        let cursor = self.inner_data_ref().cursor().0;
        let today = Local::now().date_naive();
        let year = cursor.year();
        let month = cursor.month();

        let todo = CONFIGURATION.todo_color();
        let dim = Style::from(CONFIGURATION.inactive_color());
        let reached_style = Style::from(accent).combine(Effect::Bold);
        let weekly = self.period() == GoalPeriod::Weekly;

        let archived = &self.inner_data_ref().archived_reached;
        let reached_or_archived =
            |date: NaiveDate| -> bool { self.reached_goal(date) || archived.contains(&date) };

        // How far `date` (or its week, for a weekly goal) is towards the goal.
        let ratio = |date: NaiveDate| -> f64 {
            let goal = self.goal();
            if reached_or_archived(date) {
                1.0
            } else if goal == 0 {
                0.0
            } else {
                f64::from(goal.saturating_sub(self.remaining(date))) / f64::from(goal)
            }
        };
        let progress_style = |date: NaiveDate| -> Style {
            let r = ratio(date);
            if r >= 1.0 {
                reached_style
            } else if r > 0.0 {
                Style::from(progress_color(accent, todo, r))
            } else {
                dim
            }
        };

        // Reached days out of the days up to today, for a month or a year.
        let reached_share = |days: &mut dyn Iterator<Item = NaiveDate>| -> Option<f64> {
            let (mut total, mut reached) = (0u32, 0u32);
            for date in days.filter(|&d| d <= today) {
                total += 1;
                reached += u32::from(reached_or_archived(date));
            }
            (total > 0).then(|| f64::from(reached) / f64::from(total))
        };
        let days_of =
            |y: i32, m: u32| (1..=31).filter_map(move |d| NaiveDate::from_ymd_opt(y, m, d));

        // The first line of the description sits dimmed under the title,
        // truncated to the card; `i` opens the full text in a popup.
        if let Some(first_line) = self.description().lines().next() {
            printer.with_style(dim, |p| {
                p.print(
                    (0, 0),
                    &format!(" {:.width$}", first_line, width = VIEW_WIDTH - 2),
                );
            });
        }

        // A real calendar: one column per weekday, one row per week, each cell
        // showing the value recorded for that day.
        let draw_day = |printer: &Printer| {
            const X0: usize = (VIEW_WIDTH - 21) / 2;
            for (i, name) in ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]
                .iter()
                .enumerate()
            {
                printer.with_style(dim, |p| p.print((X0 + i * 3, 1), &format!("{name:^3}")));
            }
            let lead = days_of(year, month)
                .next()
                .map_or(0, |d| d.weekday().num_days_from_monday() as usize);
            for (i, d) in days_of(year, month).enumerate() {
                let slot = lead + i;
                let coords = (X0 + (slot % 7) * 3, 2 + slot / 7);
                let (text, mut style) = match self.get_by_date(d) {
                    Some(c) => (format!("{c:^3}"), progress_style(d)),
                    None if d <= today => (format!("{:^3}", CONFIGURATION.look.missing_chr), dim),
                    None => (format!("{:^3}", CONFIGURATION.look.future_chr), dim),
                };
                if self.get_by_date(d).is_some() && ratio(d) == 0.0 {
                    style = Style::from(todo);
                }
                if d == today {
                    style = style.combine(Effect::Underline);
                }
                if d == cursor && printer.focused {
                    style = style.combine(Effect::Reverse);
                }
                printer.with_style(style, |p| p.print(coords, &text));
            }
        };

        // One bar per Mon–Sun week that touches the month.
        let draw_week = |printer: &Printer| {
            const BAR: usize = VIEW_WIDTH - 12;
            let Some(last) = days_of(year, month).next_back() else {
                return;
            };
            let mut monday = week_bounds(last.with_day(1).unwrap_or(last)).0;
            let mut line = 2;
            while monday <= last {
                let week: Vec<NaiveDate> = (0..7)
                    .filter_map(|i| monday.checked_add_days(Days::new(i)))
                    .collect();
                // A weekly goal lives on the week itself, so read it off any
                // day; a daily goal is summed over the seven days.
                let share = if weekly {
                    ratio(monday)
                } else {
                    week.iter().map(|&d| ratio(d)).sum::<f64>() / 7.0
                };
                let label_style = if week.contains(&today) {
                    Style::none()
                } else {
                    dim
                };
                printer.with_style(label_style, |p| {
                    p.print((0, line), &monday.format("%d %b").to_string());
                    if monday <= today {
                        p.print((BAR + 8, line), &format!("{:>3.0}%", share * 100.0));
                    }
                });
                let filled = (share * BAR as f64).round() as usize;
                draw_bar(printer, (7, line), BAR, filled, accent);
                line += 1;
                let Some(next) = monday.checked_add_days(Days::new(7)) else {
                    break;
                };
                monday = next;
            }
        };

        let draw_month = |printer: &Printer| {
            const MONTHS: [&str; 12] = [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ];
            const COL: usize = VIEW_WIDTH / 3;
            for (idx, name) in MONTHS.iter().enumerate() {
                let m = idx as u32 + 1;
                let coords = (idx % 3 * COL + 1, idx / 3 + 2);
                let is_current = year == today.year() && m == today.month();
                let name_style = if is_current { Style::none() } else { dim };
                printer.with_style(name_style, |p| p.print(coords, name));

                let value = (coords.0 + 4, coords.1);
                match reached_share(&mut days_of(year, m)) {
                    None => printer.with_style(dim, |p| p.print(value, "·")),
                    Some(share) => {
                        let style = if share >= 1.0 {
                            reached_style
                        } else if share > 0.0 {
                            Style::from(accent)
                        } else {
                            dim
                        };
                        printer.with_style(style, |p| {
                            p.print(value, &format!("{} {:>3.0}%", ramp(share), share * 100.0));
                        });
                    }
                }
            }
        };

        let draw_year = |printer: &Printer| {
            const BAR: usize = VIEW_WIDTH - 10;
            for i in 0..4 {
                let y = year - 3 + i;
                let line = i as usize + 2;
                let share = reached_share(&mut (1..=12).flat_map(|m| days_of(y, m)));

                let label_style = if y == today.year() {
                    Style::none()
                } else {
                    dim
                };
                printer.with_style(label_style, |p| p.print((0, line), &format!("{y}")));
                let filled = (share.unwrap_or(0.0) * BAR as f64).round() as usize;
                draw_bar(printer, (5, line), BAR, filled, accent);
                if let Some(share) = share {
                    printer.print((BAR + 6, line), &format!("{:>3.0}%", share * 100.0));
                }
            }
        };

        // A compact one-row month view: each day of the month is a single bar
        // whose height tracks that day's completion ratio. Reached days are
        // full, partial days sit mid-ramp, missed days show the lowest bar,
        // and future days are left blank. A scale of day numbers sits below.
        let draw_sparkline = |printer: &Printer| {
            for (i, d) in days_of(year, month).enumerate() {
                if d.day() == 1 || d.day() % 5 == 0 {
                    printer.with_style(dim, |p| p.print((i, 4), &d.day().to_string()));
                }
                if d > today {
                    continue;
                }
                let mut style = progress_style(d);
                if d == cursor && printer.focused {
                    style = style.combine(Effect::Reverse);
                }
                printer.with_style(style, |p| p.print((i, 3), ramp(ratio(d))));
            }
        };

        let draw_stats = |printer: &Printer| {
            // every day this habit reached its goal, including months that
            // have since been archived out of the live record
            let reached: Vec<NaiveDate> = self
                .get_dates()
                .into_iter()
                .filter(|&d| self.reached_goal(d))
                .chain(archived.iter().copied())
                .collect();

            let s = habit_stats(&reached, today, self.period());
            let unit = |n: u32| match (weekly, n == 1) {
                (true, true) => "week",
                (true, false) => "weeks",
                (false, true) => "day",
                (false, false) => "days",
            };

            let lines = [
                ("current streak", s.current_streak, unit(s.current_streak)),
                ("longest streak", s.longest_streak, unit(s.longest_streak)),
                (
                    "completed",
                    s.total,
                    if s.total == 1 { "time" } else { "times" },
                ),
                ("completion rate", s.completion_rate, "%"),
            ];
            for (i, (label, value, unit)) in lines.iter().enumerate() {
                let line = i + 2;
                let value_style = if *value > 0 { reached_style } else { dim };
                printer.with_style(dim, |p| p.print((2, line), label));
                printer.with_style(value_style, |p| {
                    p.print((18, line), &format!("{value:>4}"));
                });
                printer.print((23, line), unit);
            }
        };

        // GitHub-style contribution grid: 7 weekday rows (Mon..Sun) by a band of
        // trailing weeks. The rightmost column is the week containing the viewed
        // date, so sifting months with [ ] scrolls the heatmap through history.
        let draw_heatmap = |printer: &Printer| {
            const COLS: u64 = VIEW_WIDTH as u64;
            let cell = CONFIGURATION.look.heat_chr.to_string();
            let anchor_monday = week_bounds(cursor).0;

            for col in 0..COLS {
                let Some(week_monday) =
                    anchor_monday.checked_sub_days(Days::new((COLS - 1 - col) * 7))
                else {
                    continue;
                };
                for row in 0..7u64 {
                    let Some(date) = week_monday.checked_add_days(Days::new(row)) else {
                        continue;
                    };
                    if date > today {
                        continue; // leave future days blank
                    }
                    let coords = (col as usize, row as usize + 1);
                    printer.with_style(progress_style(date), |p| p.print(coords, &cell));
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
