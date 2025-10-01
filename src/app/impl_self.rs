use std::default::Default;
use std::f64;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::path::PathBuf;
use std::collections::HashMap;

use chrono::{Local, NaiveDate, Datelike};
use cursive::Vec2;
use cursive::direction::Absolute;

use crate::command::{Command, CommandLineError, GoalKind};
use crate::habit::{Bit, Count, Float, HabitWrapper, ViewMode};
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
        self.habits.iter().map(|x| x.name()).collect::<Vec<_>>()
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

    pub fn set_mode(&mut self, mode: ViewMode) {
        if !self.habits.is_empty() {
            self.habits[self.focus]
                .inner_data_mut_ref()
                .set_view_mode(mode);
        }
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

    pub fn load_state() -> Self {
        let regular_f = utils::habit_file();
        let read_from_file = |file: PathBuf| -> Vec<Box<dyn HabitWrapper>> {
            if let Ok(ref mut f) = File::open(file) {
                let mut j = String::new();
                f.read_to_string(&mut j);
                serde_json::from_str(&j).unwrap()
            } else {
                Vec::new()
            }
        };

        let regular = read_from_file(regular_f);
        App {
            habits: regular,
            ..Default::default()
        }
    }

    // this function does IO
    // TODO: convert this into non-blocking async function
    pub fn save_state(&self) {
        let regular: Vec<_> = self.habits.iter().collect();
        let regular_f = utils::habit_file();

        let write_to_file = |data: Vec<&Box<dyn HabitWrapper>>, file: PathBuf| {
            let mut o = serde_json::json!(data);
            o.sort_all_objects();
            let j = serde_json::to_string_pretty(&o).unwrap();
            match OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(file)
            {
                Ok(ref mut f) => f.write_all(j.as_bytes()).unwrap(),
                Err(_) => panic!("Unable to write!"),
            };
        };

        write_to_file(regular, regular_f);
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

            for date in dates {
                let month = date.month();
                let year = date.year();
                months_present
                    .entry((month, year))
                    .or_insert_with(Vec::new)
                    .push(date);
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
                        .or_insert_with(Vec::new)
                        .push(habit_json);
                }
            }
        }

        // Write archived habits to files
        let archive_path = utils::archive_dir();
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
            self.message.set_message(format!("Archived {} month(s) of habits", archived_count));
        } else {
            self.message.set_message("No old months to archive");
        }
    }

    pub fn parse_command(&mut self, result: Result<Command, CommandLineError>) {
        match result {
            Ok(c) => match c {
                Command::Add(name, goal) => {
                    if self.habits.iter().any(|x| x.name() == name) {
                        self.message.set_kind(MessageKind::Error);
                        self.message
                            .set_message(format!("Habit `{}` already exist", &name));
                        return;
                    }
                    match goal {
                        Some(GoalKind::Bit) => {
                            self.add_habit(Box::new(Bit::new(name)));
                        }
                        Some(GoalKind::Count(v)) => {
                            self.add_habit(Box::new(Count::new(name, v)));
                        }
                        Some(GoalKind::Float(v, p)) => {
                            self.message.set_kind(MessageKind::Error);
                            self.message.set_message("Added floating habit");
                            self.add_habit(Box::new(Float::new(name, v, p)));
                        }
                        _ => {
                            self.add_habit(Box::new(Count::new(name, 0)));
                        }
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
                                "a"     | "add" => "add <habit-name> [goal]     (alias: a)",
                                "aa"    | "add-auto" => "add-auto <habit-name> [goal]     (alias: aa)",
                                "d"     | "delete" => "delete <habit-name>     (alias: d)",
                                "mprev" | "month-prev" => "month-prev     (alias: mprev)",
                                "mnext" | "month-next" => "month-next     (alias: mnext)",
                                "tup"   | "track-up" => "track-up <auto-habit-name>     (alias: tup)",
                                "archive" => "archive old months to separate files",
                                "q"     | "quit" => "quit dijo",
                                "w"     | "write" => "write current state to disk   (alias: w)",
                                "h"|"?" | "help" => "help [<command>|commands|keys]     (aliases: h, ?)",
                                "cmds"  | "commands" => "add, add-auto, delete, month-{prev,next}, track-{up,down}, archive, help, quit",
                                "keys" => "TODO", // TODO (view?)
                                "wq" =>   "write current state to disk and quit dijo",
                                _ => "unknown command or help topic.",
                            }
                        )
                    } else {
                        // TODO (view?)
                        self.message.set_message("help <command>|commands|keys")
                    }
                }
                Command::Quit | Command::Write | Command::WriteAndQuit => self.save_state(),
                Command::MonthNext => self.sift_forward(),
                Command::MonthPrev => self.sift_backward(),
                Command::Archive => {
                    self.archive_habits();
                    self.save_state();
                }
                Command::Blank => {}
            },
            Err(e) => {
                self.message.set_message(e.to_string());
                self.message.set_kind(MessageKind::Error);
            }
        }
    }
}
