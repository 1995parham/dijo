use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::path::PathBuf;

use chrono::{Datelike, Days, Local, NaiveDate};
use cursive::Vec2;
use cursive::direction::Absolute;
use cursive::theme::Style;
use cursive::utils::markup::StyledString;

use crate::CONFIGURATION;
use crate::command::{Command, CommandLineError, GoalKind};
use crate::habit::{Bit, Count, Float, GoalPeriod, HabitWrapper, ViewMode};
use crate::stats::habit_stats;
use crate::utils::{self, GRID_WIDTH, VIEW_HEIGHT, VIEW_WIDTH};

use crate::app::{App, Cursor, Message, MessageKind, StatusLine};

impl App {
    pub fn new() -> Self {
        App {
            habits: vec![],
            focus: 0,
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

    pub fn missed_habits_by_name(&self, name: &str) -> Vec<String> {
        let target_habit = self.habits.iter().find(|x| x.name() == name);

        if let Some(h) = target_habit {
            h.missed_dates().iter().map(|i| i.to_string()).collect()
        } else {
            vec![]
        }
    }

    pub fn delete_by_name(&mut self, name: &str) {
        let old_len = self.habits.len();
        self.habits.retain(|h| h.name() != name);
        if old_len == self.habits.len() {
            self.message
                .set_message(format!("Could not delete habit `{name}`"))
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
                if self.focus + GRID_WIDTH < self.habits.len() - 1 {
                    self.focus += GRID_WIDTH;
                } else {
                    self.focus = self.habits.len() - 1;
                }
            }
            Absolute::Up => {
                if self.focus as isize - GRID_WIDTH as isize >= 0 {
                    self.focus -= GRID_WIDTH;
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

    /// Show the focused habit's full description in the message line. The grid
    /// cell truncates it to the column width, so this is the quick way to read a
    /// long description without opening the dashboard.
    pub fn show_focused_description(&mut self) {
        let Some(habit) = self.habits.get(self.focus) else {
            return;
        };
        let name = habit.name().to_owned();
        let description = habit.description().to_owned();
        self.message.set_kind(MessageKind::Info);
        if description.is_empty() {
            self.message
                .set_message(format!("`{name}` has no description"));
        } else {
            self.message.set_message(format!("{name}: {description}"));
        }
    }

    pub fn status(&self) -> StatusLine {
        let today = chrono::Local::now().naive_local().date();
        let remaining = self.habits.iter().map(|h| h.remaining(today)).sum::<u32>();
        let total = self.habits.iter().map(|h| h.goal()).sum::<u32>();
        let completed = total - remaining;

        let timestamp = if self.cursor.0 == today {
            format!("{}", Local::now().naive_local().date().format("%d/%b/%y"),)
        } else {
            let since = NaiveDate::signed_duration_since(today, self.cursor.0).num_days();
            let plural = if since == 1 { "" } else { "s" };
            format!("{} ({} day{} ago)", self.cursor.0, since, plural)
        };

        StatusLine(
            format!(
                "Today: {} completed, {} remaining --{}--",
                completed,
                remaining,
                self.get_mode()
            ),
            timestamp,
        )
    }

    /// Build a full-screen dashboard for the currently focused habit: a header,
    /// all-time stats, and a labelled year-long contribution heatmap. Returns
    /// the habit name (for the dialog title) and the rendered body, or `None`
    /// when there are no habits.
    pub fn focused_dashboard(&self) -> Option<(String, StyledString)> {
        let habit = self.habits.get(self.focus)?;
        let today = Local::now().date_naive();

        let reached_style = Style::from(CONFIGURATION.reached_color());
        let todo_style = Style::from(CONFIGURATION.todo_color());
        let inactive_style = Style::from(CONFIGURATION.inactive_color());

        let goal = habit.goal();
        let dates_set: HashSet<NaiveDate> = habit.get_dates().into_iter().collect();
        let archived = &habit.inner_data_ref().archived_reached;

        // A day counts as reached if it has an entry that meets the goal (i.e.
        // nothing remaining), or it was reached in a now-archived month.
        let is_reached = |d: NaiveDate| -> bool {
            (dates_set.contains(&d) && habit.remaining(d) == 0) || archived.contains(&d)
        };

        // ---- all-time stats ----
        let reached_dates: Vec<NaiveDate> = dates_set
            .iter()
            .copied()
            .filter(|&d| habit.remaining(d) == 0)
            .chain(archived.iter().copied())
            .collect();
        let s = habit_stats(&reached_dates, today, habit.period());
        let unit = if habit.period() == GoalPeriod::Weekly {
            "weeks"
        } else {
            "days"
        };

        let mut out = StyledString::new();
        if !habit.description().is_empty() {
            out.append_styled(format!("{}\n\n", habit.description()), inactive_style);
        }
        out.append_plain(format!("goal: {goal} per {}\n\n", habit.period()));
        out.append_plain(format!(
            "  current streak   {:>4} {unit}\n",
            s.current_streak
        ));
        out.append_plain(format!(
            "  longest streak   {:>4} {unit}\n",
            s.longest_streak
        ));
        out.append_plain(format!(
            "  completed        {:>4} time{}\n",
            s.total,
            if s.total == 1 { "" } else { "s" }
        ));
        out.append_plain(format!("  completion rate  {:>4} %\n\n", s.completion_rate));

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
        let mut label = " ".repeat(GUTTER);
        let mut prev_month = 0u32;
        for w in 0..WEEKS {
            let m = week_monday(w).month();
            if m != prev_month {
                let pos = GUTTER + w as usize;
                if label.len() < pos {
                    label.push_str(&" ".repeat(pos - label.len()));
                }
                label.truncate(pos);
                label.push_str(month_abbr(m));
                prev_month = m;
            }
        }
        out.append_plain(format!("{label}\n"));

        let weekday_labels = ["Mon", "   ", "Wed", "   ", "Fri", "   ", "Sun"];
        for row in 0..7u64 {
            out.append_plain(format!("{} ", weekday_labels[row as usize]));
            for w in 0..WEEKS {
                let date = week_monday(w)
                    .checked_add_days(Days::new(row))
                    .unwrap_or(start_monday);
                if date > today || date < start_monday {
                    out.append_plain(" ");
                    continue;
                }
                let (glyph, style) = if is_reached(date) {
                    ("█", reached_style)
                } else if goal > 0 && habit.remaining(date) < goal {
                    ("▒", todo_style) // some progress, goal not met
                } else {
                    ("░", inactive_style) // missed or no data
                };
                out.append_styled(glyph, style);
            }
            out.append_plain("\n");
        }

        // ---- legend ----
        out.append_plain("\n  ");
        out.append_styled("█", reached_style);
        out.append_plain(" done   ");
        out.append_styled("▒", todo_style);
        out.append_plain(" partial   ");
        out.append_styled("░", inactive_style);
        out.append_plain(" missed");

        Some((habit.name().to_owned(), out))
    }

    pub fn max_size(&self) -> Vec2 {
        let width = GRID_WIDTH * VIEW_WIDTH;
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

    pub fn load_state() -> Result<Self, String> {
        let regular_f = utils::habit_file()?;
        let read_from_file = |file: PathBuf| -> Result<Vec<Box<dyn HabitWrapper>>, String> {
            match File::open(file) {
                Ok(ref mut f) => {
                    let mut j = String::new();
                    f.read_to_string(&mut j)
                        .map_err(|e| format!("Failed to read habit file: `{e}`"))?;
                    serde_json::from_str(&j)
                        .map_err(|e| format!("Failed to parse habit file: `{e}`"))
                }
                // No file yet: a fresh start, not an error.
                Err(_) => Ok(Vec::new()),
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

        // Write to a sibling temp file, then atomically rename it over the
        // target. A crash mid-write leaves the original file untouched instead
        // of truncated.
        let file_name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("habit_record.json");
        let tmp = file.with_file_name(format!("{file_name}.tmp"));

        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)
            .map_err(|e| format!("could not open habit file for writing: {e}"))?;
        f.write_all(j.as_bytes())
            .map_err(|e| format!("could not write habit file: {e}"))?;
        std::fs::rename(&tmp, &file).map_err(|e| format!("could not save habit file: {e}"))?;

        Ok(())
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

        // Write archived habits to files
        let archive_path = match utils::archive_dir() {
            Ok(p) => p,
            Err(e) => {
                self.message.set_kind(MessageKind::Error);
                self.message.set_message(e);
                return;
            }
        };
        let mut archived_count = 0;

        for ((month, year), habits) in habits_by_month.iter() {
            // Create a date for this month to get the month name
            let date = NaiveDate::from_ymd_opt(*year, *month, 1)
                .unwrap_or_else(|| NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
            let month_name = date.format("%b").to_string().to_lowercase();

            let filename = format!("{}_{}.json", month_name, year);
            let mut file_path = archive_path.clone();
            file_path.push(filename);

            let mut o = serde_json::json!(habits);
            o.sort_all_objects();
            let j = serde_json::to_string_pretty(&o).unwrap();

            match OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(file_path)
            {
                Ok(ref mut f) => {
                    f.write_all(j.as_bytes()).unwrap();
                    archived_count += 1;
                }
                Err(_) => {
                    self.message.set_kind(MessageKind::Error);
                    self.message.set_message("Failed to write archive file");
                    return;
                }
            }
        }

        // Update habits with current month data only
        self.habits = serde_json::from_value(serde_json::Value::Array(current_month_habits))
            .unwrap_or_else(|_| Vec::new());

        if archived_count > 0 {
            self.message
                .set_message(format!("Archived {} month(s) of habits", archived_count));
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
                            .set_message(format!("Habit `{name}` already exist"));
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
                        _ => {
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
                Command::Delete(name) => {
                    self.delete_by_name(&name);
                    self.focus = 0;
                }
                Command::Help(input) => {
                    if let Some(topic) = input.as_ref().map(String::as_ref) {
                        self.message.set_message(
                            match topic {
                                "a"     | "add" => "add <habit-name> [goal[/week]]   e.g. `add gym 3/week`  (alias: a)",
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
                                "keys" => "hjkl: move | HJKL: cursor | n/Enter: +1 | p/BS: -1 | v: cycle view (day/week/month/sparkline/year/stats/heatmap) | d: dashboard | i: show full description | []: month | Esc: reset",
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

fn month_abbr(month: u32) -> &'static str {
    const NAMES: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    NAMES.get((month as usize).wrapping_sub(1)).unwrap_or(&"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::habit::{Count, Habit};

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
    fn show_focused_description_reports_full_text() {
        let mut app = App::new();
        app.add_habit(Box::new(Count::new("read", 1)));
        app.parse_command(Ok(Command::Describe(
            "read".into(),
            "a chapter each and every single night before bed".into(),
        )));

        app.show_focused_description();
        assert_eq!(
            app.message.contents(),
            "read: a chapter each and every single night before bed"
        );
    }

    #[test]
    fn show_focused_description_notes_when_empty() {
        let mut app = App::new();
        app.add_habit(Box::new(Count::new("read", 1)));

        app.show_focused_description();
        assert!(app.message.contents().contains("no description"));
    }

    #[test]
    fn describe_unknown_habit_is_an_error() {
        let mut app = App::new();
        app.parse_command(Ok(Command::Describe("ghost".into(), "boo".into())));
        assert!(matches!(app.message.kind(), MessageKind::Error));
    }

    #[test]
    fn month_abbr_is_one_based_and_bounded() {
        assert_eq!(month_abbr(1), "Jan");
        assert_eq!(month_abbr(12), "Dec");
        assert_eq!(month_abbr(0), "");
        assert_eq!(month_abbr(13), "");
    }
}
