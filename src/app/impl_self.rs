use std::collections::{HashMap, HashSet};
use std::io::ErrorKind;
use std::path::PathBuf;

use chrono::{Datelike, Days, Local, NaiveDate};
use cursive::direction::Absolute;
use cursive::theme::{Effect, Style};
use cursive::utils::markup::StyledString;

use crate::CONFIGURATION;
use crate::command::{Command, CommandLineError, GoalKind};
use crate::habit::{Bit, Count, Float, GoalPeriod, HabitWrapper, Limit, TrackEvent, ViewMode};
use crate::stats::habit_stats;
use crate::utils::{self, GRID_WIDTH, VIEW_HEIGHT};
use crate::views::{day_style, progress_ratio};

use crate::app::{App, Cursor, Message, MessageKind, StatusLine};

impl App {
    pub fn new() -> Self {
        App {
            habits: vec![],
            focus: 0,
            cols: GRID_WIDTH,
            cursor: Cursor::new(),
            message: Message::startup(),
        }
    }

    pub fn add_habit(&mut self, h: Box<dyn HabitWrapper>) {
        self.habits.push(h);
    }

    pub fn list_habits(&self) -> Vec<String> {
        self.habits.iter().map(|x| x.name().to_owned()).collect()
    }

    /// The days of this month with no entry for the habit called `name`, or
    /// `None` when there is no such habit.
    pub fn missed_habits_by_name(&self, name: &str) -> Option<Vec<String>> {
        let habit = self.habits.iter().find(|x| x.name() == name)?;
        Some(habit.missed_dates().iter().map(|i| i.to_string()).collect())
    }

    /// Delete the habit called `name`. The focus moves to the first habit only
    /// when a habit was deleted; an unknown name is an error.
    pub fn delete_by_name(&mut self, name: &str) {
        let old_len = self.habits.len();
        self.habits.retain(|h| h.name() != name);
        if old_len == self.habits.len() {
            self.message.set_kind(MessageKind::Error);
            self.message
                .set_message(format!("Habit `{name}` does not exist"));
        } else {
            self.focus = 0;
        }
    }

    pub fn get_mode(&self) -> ViewMode {
        if self.habits.is_empty() {
            return ViewMode::Day;
        }

        self.habits[self.focus].inner_data_ref().view_mode()
    }

    pub fn sift_backward(&mut self) {
        self.cursor.month_backward();
        for v in self.habits.iter_mut() {
            v.inner_data_mut_ref().cursor.month_backward();
        }
    }

    pub fn sift_forward(&mut self) {
        self.cursor.month_forward();
        for v in self.habits.iter_mut() {
            v.inner_data_mut_ref().cursor.month_forward();
        }
    }

    pub fn reset_cursor(&mut self) {
        self.cursor.reset();
        for v in self.habits.iter_mut() {
            v.inner_data_mut_ref().cursor.reset();
        }
    }

    pub fn move_cursor(&mut self, d: Absolute) {
        self.cursor.small_seek(d);
        for v in self.habits.iter_mut() {
            v.inner_data_mut_ref().move_cursor(d);
        }
    }

    pub fn set_focus(&mut self, d: Absolute) {
        if self.habits.is_empty() {
            return;
        }
        match d {
            Absolute::Right => {
                if self.focus != self.habits.len() - 1 {
                    self.focus += 1;
                }
            }
            Absolute::Left => {
                if self.focus != 0 {
                    self.focus -= 1;
                }
            }
            Absolute::Down => {
                if self.focus + self.cols < self.habits.len() - 1 {
                    self.focus += self.cols;
                } else {
                    self.focus = self.habits.len() - 1;
                }
            }
            Absolute::Up => {
                if self.focus >= self.cols {
                    self.focus -= self.cols;
                } else {
                    self.focus = 0;
                }
            }
            Absolute::None => {}
        }
    }

    pub fn clear_message(&mut self) {
        self.message.clear();
    }

    /// Name and full description of the focused habit, for the `i` popup. The
    /// grid cell truncates the description to one line, so the popup is where
    /// long or multi-line text is read in full. Returns `None` when there are
    /// no habits, or when the habit has no description (noted in the message
    /// line instead).
    pub fn focused_description(&mut self) -> Option<(String, String)> {
        let habit = self.habits.get(self.focus)?;
        let name = habit.name().to_owned();
        let description = habit.description().to_owned();
        if description.is_empty() {
            self.message.set_kind(MessageKind::Info);
            self.message
                .set_message(format!("`{name}` has no description"));
            return None;
        }
        Some((name, description))
    }

    pub fn status(&self) -> StatusLine {
        let today = Local::now().date_naive();
        let with_goal = || self.habits.iter().filter(|h| h.goal() > 0);

        let timestamp = if self.cursor.0 == today {
            today.format("%a %d %b %Y").to_string()
        } else {
            let since = NaiveDate::signed_duration_since(today, self.cursor.0).num_days();
            let plural = if since == 1 { "" } else { "s" };
            format!("{} ({} day{} ago)", self.cursor.0, since, plural)
        };

        StatusLine {
            mode: self.get_mode(),
            done: with_goal().filter(|h| h.remaining(today) == 0).count(),
            total: with_goal().count(),
            timestamp,
        }
    }

    /// Add `amount` to the entry of the habit `name` on `date`, for `dijo
    /// track`. A yes/no habit is set to yes (a negative amount sets it to no),
    /// and `0` on an empty day of a limit habit records a clean day.
    pub fn track(&mut self, name: &str, amount: &str, date: NaiveDate) -> Result<(), String> {
        let habit = self
            .habits
            .iter_mut()
            .find(|h| h.name() == name)
            .ok_or_else(|| format!("habit `{name}` does not exist"))?;
        let steps = track_steps(&habit.kind(), amount)?;
        match habit.kind() {
            GoalKind::Bit => {
                if steps > 0 && habit.remaining(date) > 0 {
                    habit.modify(date, TrackEvent::Increment);
                } else if steps < 0 && habit.has_entry(date) {
                    habit.modify(date, TrackEvent::Decrement);
                }
            }
            GoalKind::Limit(_) if steps == 0 && !habit.has_entry(date) => {
                habit.modify(date, TrackEvent::Decrement);
            }
            _ => {
                let event = if steps < 0 {
                    TrackEvent::Decrement
                } else {
                    TrackEvent::Increment
                };
                for _ in 0..steps.unsigned_abs() {
                    habit.modify(date, event);
                }
            }
        }
        Ok(())
    }

    /// Today's progress of every habit with a goal, for `dijo status`: a line
    /// such as `3/5 done today`, or JSON that a waybar custom module reads
    /// (`text`, `tooltip`, `class`) with the details of each habit.
    pub fn status_report(&self, json: bool) -> String {
        let today = Local::now().date_naive();
        let habits: Vec<_> = self.habits.iter().filter(|h| h.goal() > 0).collect();
        let done = habits.iter().filter(|h| h.remaining(today) == 0).count();
        let text = format!("{done}/{} done today", habits.len());
        if !json {
            return text;
        }
        let tooltip = habits
            .iter()
            .map(|h| {
                let mark = if h.remaining(today) == 0 { "✓" } else { "·" };
                format!("{mark} {}", h.name())
            })
            .collect::<Vec<_>>()
            .join("\n");
        let class = if done == habits.len() { "done" } else { "todo" };
        let details: Vec<_> = habits
            .iter()
            .map(|h| {
                serde_json::json!({
                    "name": h.name(),
                    "done": h.remaining(today) == 0,
                    "goal": h.goal(),
                    "remaining": h.remaining(today),
                    "period": h.period().to_string(),
                })
            })
            .collect();
        serde_json::json!({
            "text": text,
            "tooltip": tooltip,
            "class": class,
            "done": done,
            "total": habits.len(),
            "habits": details,
        })
        .to_string()
    }

    /// Build a full-screen dashboard for the currently focused habit: a header,
    /// all-time stats, and a labelled year-long contribution heatmap. Returns
    /// the habit name (for the dialog title) and the rendered body, or `None`
    /// when there are no habits.
    pub fn focused_dashboard(&self) -> Option<(String, StyledString)> {
        let habit = self.habits.get(self.focus)?;
        let today = Local::now().date_naive();

        let accent = CONFIGURATION.accent(self.focus);
        let reached_style = Style::from(accent).combine(Effect::Bold);
        let inactive_style = Style::from(CONFIGURATION.inactive_color());

        let goal = habit.goal();
        let dates_set: HashSet<NaiveDate> = habit.get_dates().into_iter().collect();
        let archived = &habit.inner_data_ref().archived_reached;

        let weekly = habit.period() == GoalPeriod::Weekly;

        // A day counts as reached if it has an entry that meets the goal (i.e.
        // nothing remaining), or it was reached in a now-archived month. A day
        // with an entry but no progress is undone, as in the card views.
        let is_reached = |d: NaiveDate| -> bool {
            (dates_set.contains(&d) && habit.remaining(d) == 0) || archived.contains(&d)
        };
        let ratio = |d: NaiveDate| progress_ratio(is_reached(d), goal, habit.remaining(d));
        let undone = |d: NaiveDate| !weekly && dates_set.contains(&d) && ratio(d) == 0.0;

        // ---- all-time stats ----
        let reached_dates: Vec<NaiveDate> = dates_set
            .iter()
            .copied()
            .filter(|&d| habit.remaining(d) == 0)
            .chain(archived.iter().copied())
            .collect();
        let s = habit_stats(&reached_dates, today, habit.period());
        let unit = if weekly { "weeks" } else { "days" };

        let mut out = StyledString::new();
        if !habit.description().is_empty() {
            out.append_styled(format!("{}\n\n", habit.description()), inactive_style);
        }
        out.append_styled("  goal             ", inactive_style);
        out.append_plain(format!("{goal:>4} per {}\n\n", habit.period()));
        let times = if s.total == 1 { "time" } else { "times" };
        for (label, value, unit) in [
            ("current streak", s.current_streak, unit),
            ("longest streak", s.longest_streak, unit),
            ("completed", s.total, times),
            ("completion rate", s.completion_rate, "%"),
        ] {
            out.append_styled(format!("  {label:<17}"), inactive_style);
            let style = if value > 0 {
                reached_style
            } else {
                inactive_style
            };
            out.append_styled(format!("{value:>4}"), style);
            out.append_plain(format!(" {unit}\n"));
        }
        out.append_plain("\n");

        // ---- trailing-year heatmap ----
        const WEEKS: u64 = 53;
        const GUTTER: usize = 4; // width of the weekday-label column
        let weekday_off = today.weekday().num_days_from_monday() as u64;
        let anchor_monday = today
            .checked_sub_days(Days::new(weekday_off))
            .unwrap_or(today);
        let start_monday = anchor_monday
            .checked_sub_days(Days::new((WEEKS - 1) * 7))
            .unwrap_or(anchor_monday);

        let week_monday = |w: u64| {
            start_monday
                .checked_add_days(Days::new(w * 7))
                .unwrap_or(start_monday)
        };

        // month-label row: drop each month's abbreviation at the column where
        // that month first appears
        let mut starts: Vec<(usize, u32)> = Vec::new();
        for w in 0..WEEKS {
            let m = week_monday(w).month();
            if starts.last().is_none_or(|&(_, prev)| prev != m) {
                starts.push((GUTTER + w as usize, m));
            }
        }
        let mut label = String::new();
        for (i, &(pos, m)) in starts.iter().enumerate() {
            // a month with too few columns before the next one gets no label
            if starts.get(i + 1).is_some_and(|&(next, _)| next < pos + 4) {
                continue;
            }
            label.push_str(&" ".repeat(pos.saturating_sub(label.len())));
            label.push_str(month_abbr(m));
        }
        out.append_styled(format!("{label}\n"), inactive_style);

        let cell = CONFIGURATION.look.heat_chr.to_string();
        let weekday_labels = ["Mon", "   ", "Wed", "   ", "Fri", "   ", "Sun"];
        for row in 0..7u64 {
            out.append_styled(format!("{} ", weekday_labels[row as usize]), inactive_style);
            for w in 0..WEEKS {
                let date = week_monday(w)
                    .checked_add_days(Days::new(row))
                    .unwrap_or(start_monday);
                if date > today || date < start_monday {
                    out.append_plain(" ");
                    continue;
                }
                out.append_styled(cell.as_str(), day_style(ratio(date), undone(date), accent));
            }
            out.append_plain("\n");
        }

        // ---- legend ----
        out.append_plain("\n   ");
        for (style, name) in [
            (day_style(1.0, false, accent), "done"),
            (day_style(0.5, false, accent), "partial"),
            (day_style(0.0, true, accent), "undone"),
            (day_style(0.0, false, accent), "no entry"),
        ] {
            out.append_plain(" ");
            out.append_styled(cell.as_str(), style);
            out.append_plain(format!(" {name}  "));
        }

        Some((habit.name().to_owned(), out))
    }

    /// Width of the whole view. It is at least two cards wide when the
    /// terminal has room for two, so the status line fits beside one habit.
    pub fn frame_width(&self) -> usize {
        let used = self.cols.min(self.habits.len().max(1));
        utils::grid_width(used.max(self.cols.min(2)))
    }

    /// Height of the card grid, without the status and message lines.
    pub fn grid_height(&self) -> usize {
        if self.habits.is_empty() {
            return 4; // room for the "no habits yet" hint
        }
        self.habits.len().div_ceil(self.cols) * VIEW_HEIGHT
    }

    pub fn load_state() -> Result<Self, String> {
        let regular_f = utils::habit_file()?;
        let read_from_file = |file: PathBuf| -> Result<Vec<Box<dyn HabitWrapper>>, String> {
            match std::fs::read_to_string(file) {
                Ok(j) => serde_json::from_str(&j)
                    .map_err(|e| format!("Failed to parse habit file: `{e}`")),
                // No file yet: a fresh start, not an error.
                Err(e) if e.kind() == ErrorKind::NotFound => Ok(Vec::new()),
                // Any other failure must not look like an empty record, or the
                // save on exit would replace the real one.
                Err(e) => Err(format!("Failed to read habit file: `{e}`")),
            }
        };

        let mut regular = read_from_file(regular_f)?;

        let archived = utils::load_archived_reached_goals();
        for habit in regular.iter_mut() {
            if let Some(dates) = archived.get(habit.name()) {
                habit.inner_data_mut_ref().archived_reached = dates.clone();
            }
        }

        Ok(App {
            habits: regular,
            ..Default::default()
        })
    }

    pub fn save_state(&self) -> Result<(), String> {
        let regular: Vec<_> = self.habits.iter().collect();
        let file = utils::habit_file()?;

        let mut o = serde_json::json!(regular);
        o.sort_all_objects();
        let j = serde_json::to_string_pretty(&o)
            .map_err(|e| format!("could not serialize habits: {e}"))?;

        utils::write_atomic(&file, j.as_bytes())
    }

    pub fn archive_habits(&mut self) {
        let today = Local::now().date_naive();
        let current_month = today.month();
        let current_year = today.year();

        // Group JSON habits by month
        let mut habits_by_month: HashMap<(u32, i32), Vec<serde_json::Value>> = HashMap::new();
        let mut current_month_habits: Vec<serde_json::Value> = Vec::new();

        // Process each habit: check dates in Rust first, then serialize
        for habit in self.habits.iter() {
            // Get all dates from the habit using the trait method
            let dates = habit.get_dates();

            // Check which months this habit's stats belong to
            let mut months_present: HashMap<(u32, i32), Vec<NaiveDate>> = HashMap::new();
            let mut has_current_month = false;

            for date in dates {
                let month = date.month();
                let year = date.year();

                if month == current_month && year == current_year {
                    has_current_month = true;
                }

                months_present.entry((month, year)).or_default().push(date);
            }

            // For each month, create a habit with only that month's stats
            for ((month, year), month_dates) in months_present {
                let mut habit_json = serde_json::to_value(&**habit).unwrap();

                // Filter the stats to only include dates for this month
                if let Some(stats) = habit_json.get_mut("stats").and_then(|s| s.as_object_mut()) {
                    stats.retain(|date_str, _| {
                        if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                            month_dates.contains(&date)
                        } else {
                            false
                        }
                    });
                }

                if month == current_month && year == current_year {
                    current_month_habits.push(habit_json);
                } else {
                    habits_by_month
                        .entry((month, year))
                        .or_default()
                        .push(habit_json);
                }
            }

            // If habit has no current month data, add it with empty stats
            if !has_current_month {
                let mut habit_json = serde_json::to_value(&**habit).unwrap();

                // Clear the stats
                if let Some(stats) = habit_json.get_mut("stats").and_then(|s| s.as_object_mut()) {
                    stats.clear();
                }

                current_month_habits.push(habit_json);
            }
        }

        // Write archived habits to files. Nothing is removed from the live
        // record until every archive file is safely on disk.
        let written = utils::archive_dir().and_then(|archive_path| {
            for ((month, year), habits) in &habits_by_month {
                // Create a date for this month to get the month name
                let date = NaiveDate::from_ymd_opt(*year, *month, 1)
                    .ok_or_else(|| format!("invalid month {month}/{year}"))?;
                let month_name = date.format("%b").to_string().to_lowercase();
                let file_path = archive_path.join(format!("{month_name}_{year}.json"));

                // A month that was archived before keeps its entries: the new
                // ones are merged into the file instead of replacing it.
                let existing = match std::fs::read_to_string(&file_path) {
                    Ok(j) => serde_json::from_str(&j).map_err(|e| {
                        format!("could not parse archive {}: {e}", file_path.display())
                    })?,
                    Err(e) if e.kind() == ErrorKind::NotFound => Vec::new(),
                    Err(e) => {
                        return Err(format!(
                            "could not read archive {}: {e}",
                            file_path.display()
                        ));
                    }
                };

                let mut o = serde_json::json!(utils::merge_archive(existing, habits.clone()));
                o.sort_all_objects();
                let j = serde_json::to_string_pretty(&o)
                    .map_err(|e| format!("could not serialize archive: {e}"))?;
                utils::write_atomic(&file_path, j.as_bytes())?;
            }
            Ok(habits_by_month.len())
        });
        let archived_count = match written {
            Ok(n) => n,
            Err(e) => {
                self.message.set_kind(MessageKind::Error);
                self.message.set_message(e);
                return;
            }
        };

        // Update habits with current month data only. The habits keep their
        // order, so each one takes its view state back by position.
        let trimmed: Vec<Box<dyn HabitWrapper>> =
            match serde_json::from_value(serde_json::Value::Array(current_month_habits)) {
                Ok(h) => h,
                Err(e) => {
                    self.message.set_kind(MessageKind::Error);
                    self.message
                        .set_message(format!("could not trim the archived habits: {e}"));
                    return;
                }
            };
        let archived = utils::load_archived_reached_goals();
        let old = std::mem::replace(&mut self.habits, trimmed);
        for (habit, old) in self.habits.iter_mut().zip(old) {
            let dates = archived.get(habit.name()).cloned();
            let data = habit.inner_data_mut_ref();
            data.cursor = old.inner_data_ref().cursor;
            data.view_mode = old.inner_data_ref().view_mode;
            if let Some(dates) = dates {
                data.archived_reached = dates;
            }
        }

        if archived_count > 0 {
            self.message
                .set_message(format!("Archived {archived_count} month(s) of habits"));
        } else {
            self.message.set_message("No old months to archive");
        }
    }

    pub fn parse_command(&mut self, result: Result<Command, CommandLineError>) {
        match result {
            Ok(c) => match c {
                Command::Add(name, goal, period) => {
                    if self.habits.iter().any(|x| x.name() == name) {
                        self.message.set_kind(MessageKind::Error);
                        self.message
                            .set_message(format!("Habit `{name}` already exists"));
                        return;
                    }
                    match goal {
                        Some(GoalKind::Bit) => {
                            self.add_habit(Box::new(Bit::new(name)));
                        }
                        Some(GoalKind::Count(v)) => {
                            self.add_habit(Box::new(Count::new(name, v).with_period(period)));
                        }
                        Some(GoalKind::Float(v, p)) => {
                            self.add_habit(Box::new(Float::new(name, v, p).with_period(period)));
                        }
                        Some(GoalKind::Limit(_)) if period == GoalPeriod::Weekly => {
                            self.message.set_kind(MessageKind::Error);
                            self.message
                                .set_message("A limit (`<n`) is per day; `/week` does not apply");
                        }
                        Some(GoalKind::Limit(v)) => {
                            self.add_habit(Box::new(Limit::new(name, v)));
                        }
                        None => {
                            self.add_habit(Box::new(Count::new(name, 0)));
                        }
                    }
                }
                Command::Describe(name, description) => {
                    if let Some(habit) = self.habits.iter_mut().find(|h| h.name() == name) {
                        habit.set_description(description);
                    } else {
                        self.message.set_kind(MessageKind::Error);
                        self.message
                            .set_message(format!("Habit `{name}` does not exist"));
                    }
                }
                Command::Delete(name) => self.delete_by_name(&name),
                Command::Help(input) => {
                    if let Some(topic) = input.as_ref().map(String::as_ref) {
                        self.message.set_message(
                            match topic {
                                "a"     | "add" => "add <habit-name> [goal[/week] | <limit]   e.g. `add gym 3/week`, `add smoke <5`  (alias: a)",
                                "describe" | "desc" => "describe <habit-name> <text...>     (alias: desc)",
                                "d"     | "delete" => "delete <habit-name>     (alias: d)",
                                "mprev" | "month-prev" => "month-prev     (alias: mprev)",
                                "mnext" | "month-next" => "month-next     (alias: mnext)",
                                "archive" => "archive old months to separate files",
                                "dashboard" | "dash" => "open the focused habit's dashboard     (alias: dash, key: d)",
                                "q"     | "quit" => "quit dijo",
                                "w"     | "write" => "write current state to disk   (alias: w)",
                                "h"|"?" | "help" => "help [<command>|commands|keys]     (aliases: h, ?)",
                                "cmds"  | "commands" => "add, describe, delete, month-{prev,next}, archive, dashboard, help, quit",
                                "keys" => "hjkl: move | HJKL: cursor | n/Enter: +1 | p/BS: -1 | v: cycle view (day/week/month/sparkline/year/stats/heatmap) | d: dashboard | i: description popup | []: month | Esc: reset",
                                "wq" =>   "write current state to disk and quit dijo",
                                _ => "unknown command or help topic.",
                            }
                        )
                    } else {
                        self.message.set_message("help <command>|commands|keys")
                    }
                }
                Command::Quit | Command::Write | Command::WriteAndQuit => {
                    if let Err(e) = self.save_state() {
                        self.message.set_kind(MessageKind::Error);
                        self.message.set_message(e);
                    }
                }
                Command::MonthNext => self.sift_forward(),
                Command::MonthPrev => self.sift_backward(),
                Command::Archive => {
                    self.archive_habits();
                    if let Err(e) = self.save_state() {
                        self.message.set_kind(MessageKind::Error);
                        self.message.set_message(e);
                    }
                }
                // opening the dashboard needs access to the Cursive root, so it
                // is handled in command::call_on_app, not here.
                Command::Dashboard => {}
                Command::Blank => {}
            },
            Err(e) => {
                self.message.set_message(e.to_string());
                self.message.set_kind(MessageKind::Error);
            }
        }
    }
}

/// The number of steps that `amount` makes for a habit of `kind`: whole units
/// for a count, units of the habit's precision for a float (`2.5` with one
/// decimal is 25 steps).
fn track_steps(kind: &GoalKind, amount: &str) -> Result<i64, String> {
    let invalid = || format!("invalid amount `{amount}`");
    match kind {
        GoalKind::Float(_, precision) => {
            let value: f64 = amount.parse().map_err(|_| invalid())?;
            let steps = value * 10f64.powi(i32::from(*precision));
            if !steps.is_finite() || steps.abs() > f64::from(u32::MAX) {
                return Err(invalid());
            }
            Ok(steps.round() as i64)
        }
        _ => amount.parse().map_err(|_| invalid()),
    }
}

fn month_abbr(month: u32) -> &'static str {
    const NAMES: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    NAMES.get((month as usize).wrapping_sub(1)).unwrap_or(&"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::habit::{Bit, Count, Float, Habit, Limit};

    #[test]
    fn dashboard_is_none_without_habits() {
        assert!(App::new().focused_dashboard().is_none());
    }

    #[test]
    fn dashboard_renders_for_focused_habit() {
        let mut app = App::new();
        let mut habit = Count::new("read", 1);
        Habit::insert_entry(&mut habit, Local::now().date_naive(), 1);
        app.add_habit(Box::new(habit));

        let (name, body) = app
            .focused_dashboard()
            .expect("dashboard for focused habit");
        assert_eq!(name, "read");
        // header, stats and legend are all present in the rendered body
        assert!(body.source().contains("current streak"));
        assert!(body.source().contains("completion rate"));
        assert!(body.source().contains("undone"));
    }

    #[test]
    fn describe_sets_description_and_shows_in_dashboard() {
        let mut app = App::new();
        app.add_habit(Box::new(Count::new("read", 1)));

        app.parse_command(Ok(Command::Describe(
            "read".into(),
            "a chapter each night".into(),
        )));

        let (_, body) = app.focused_dashboard().expect("dashboard");
        assert!(body.source().contains("a chapter each night"));
    }

    #[test]
    fn focused_description_returns_full_multiline_text() {
        let mut app = App::new();
        app.add_habit(Box::new(Count::new("read", 1)));
        app.parse_command(Ok(Command::Describe(
            "read".into(),
            "a chapter each night\nbefore bed".into(),
        )));

        let (name, body) = app.focused_description().expect("description popup body");
        assert_eq!(name, "read");
        assert_eq!(body, "a chapter each night\nbefore bed");
    }

    #[test]
    fn focused_description_is_none_without_habits() {
        assert!(App::new().focused_description().is_none());
    }

    #[test]
    fn focused_description_notes_when_empty() {
        let mut app = App::new();
        app.add_habit(Box::new(Count::new("read", 1)));

        assert!(app.focused_description().is_none());
        assert!(app.message.contents().contains("no description"));
    }

    #[test]
    fn describe_unknown_habit_is_an_error() {
        let mut app = App::new();
        app.parse_command(Ok(Command::Describe("ghost".into(), "boo".into())));
        assert!(matches!(app.message.kind(), MessageKind::Error));
    }

    #[test]
    fn a_new_command_clears_the_error_kind() {
        let mut app = App::new();
        app.parse_command(Err(CommandLineError::InvalidCommand("nope".into())));
        assert!(matches!(app.message.kind(), MessageKind::Error));

        app.clear_message();
        app.parse_command(Ok(Command::Help(None)));
        assert!(matches!(app.message.kind(), MessageKind::Info));
    }

    #[test]
    fn a_limit_goal_adds_a_limit_habit() {
        let mut app = App::new();
        app.parse_command("add smoke <5".parse());
        assert_eq!(app.list_habits(), vec!["smoke".to_string()]);
        assert_eq!(app.habits[0].kind(), GoalKind::Limit(5));
    }

    #[test]
    fn a_weekly_limit_is_rejected() {
        let mut app = App::new();
        app.parse_command("add smoke <5/week".parse());
        assert!(app.habits.is_empty());
        assert!(matches!(app.message.kind(), MessageKind::Error));
    }

    #[test]
    fn deleting_an_unknown_habit_is_an_error_and_keeps_the_focus() {
        let mut app = App::new();
        app.add_habit(Box::new(Count::new("read", 1)));
        app.add_habit(Box::new(Count::new("gym", 1)));
        app.focus = 1;

        app.parse_command(Ok(Command::Delete("ghost".into())));
        assert_eq!(app.focus, 1);
        assert!(matches!(app.message.kind(), MessageKind::Error));

        app.parse_command(Ok(Command::Delete("gym".into())));
        assert_eq!(app.focus, 0);
        assert_eq!(app.list_habits(), vec!["read".to_string()]);
    }

    fn tracked_app() -> (App, NaiveDate) {
        let mut app = App::new();
        app.add_habit(Box::new(Bit::new("read")));
        app.add_habit(Box::new(Count::new("water", 8)));
        app.add_habit(Box::new(Float::new("run", 50, 1)));
        app.add_habit(Box::new(Limit::new("smoke", 2)));
        (app, Local::now().date_naive())
    }

    #[test]
    fn track_adds_to_a_count_and_a_float() {
        let (mut app, today) = tracked_app();
        app.track("water", "3", today).unwrap();
        app.track("water", "-1", today).unwrap();
        assert_eq!(app.habits[1].remaining(today), 6);
        app.track("run", "2.5", today).unwrap();
        assert_eq!(app.habits[2].remaining(today), 25);
    }

    #[test]
    fn track_sets_a_yes_no_habit_without_a_toggle() {
        let (mut app, today) = tracked_app();
        app.track("read", "1", today).unwrap();
        app.track("read", "1", today).unwrap();
        assert_eq!(app.habits[0].remaining(today), 0, "still yes");
        app.track("read", "-1", today).unwrap();
        assert_eq!(app.habits[0].remaining(today), 1);
    }

    #[test]
    fn track_zero_records_a_clean_day_of_a_limit() {
        let (mut app, today) = tracked_app();
        app.track("smoke", "0", today).unwrap();
        assert!(app.habits[3].has_entry(today));
        assert_eq!(app.habits[3].remaining(today), 0);
    }

    #[test]
    fn track_rejects_an_unknown_habit_and_a_bad_amount() {
        let (mut app, today) = tracked_app();
        assert!(app.track("ghost", "1", today).is_err());
        assert!(app.track("water", "lots", today).is_err());
        assert!(app.track("run", "1e30", today).is_err());
    }

    #[test]
    fn status_report_counts_the_habits_done_today() {
        let (mut app, today) = tracked_app();
        app.track("read", "1", today).unwrap();
        app.track("smoke", "0", today).unwrap();
        assert_eq!(app.status_report(false), "2/4 done today");

        let json: serde_json::Value = serde_json::from_str(&app.status_report(true)).unwrap();
        assert_eq!(json["text"], "2/4 done today");
        assert_eq!(json["class"], "todo");
        assert_eq!(json["habits"][0]["done"], true);
        assert_eq!(json["habits"][1]["remaining"], 8);
    }

    #[test]
    fn month_abbr_is_one_based_and_bounded() {
        assert_eq!(month_abbr(1), "Jan");
        assert_eq!(month_abbr(12), "Dec");
        assert_eq!(month_abbr(0), "");
        assert_eq!(month_abbr(13), "");
    }
}
