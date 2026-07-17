use chrono::{Datelike, Days, NaiveDate};
use cursive::theme::{BaseColor, Color};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

pub const VIEW_WIDTH: usize = 30;
pub const VIEW_HEIGHT: usize = 10;
pub const GRID_WIDTH: usize = 3;

/// The Monday and Sunday bounding the ISO-style week that contains `date`.
/// Used by weekly-goal habits to aggregate a week's entries.
pub fn week_bounds(date: NaiveDate) -> (NaiveDate, NaiveDate) {
    let offset = date.weekday().num_days_from_monday() as u64;
    let monday = date.checked_sub_days(Days::new(offset)).unwrap_or(date);
    let sunday = monday.checked_add_days(Days::new(6)).unwrap_or(monday);
    (monday, sunday)
}

#[derive(Serialize, Deserialize)]
pub struct Characters {
    #[serde(default = "base_char")]
    pub true_chr: char,
    #[serde(default = "base_char")]
    pub false_chr: char,
    #[serde(default = "base_char")]
    pub future_chr: char,
    #[serde(default = "base_char")]
    pub missing_chr: char,
}

fn base_char() -> char {
    '·'
}

impl Default for Characters {
    fn default() -> Self {
        Characters {
            true_chr: '+',
            false_chr: '-',
            future_chr: '.',
            missing_chr: '?',
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Colors {
    #[serde(default = "cyan")]
    pub reached: String,
    #[serde(default = "magenta")]
    pub todo: String,
    #[serde(default = "light_black")]
    pub inactive: String,
}

fn cyan() -> String {
    "cyan".into()
}
fn magenta() -> String {
    "magenta".into()
}
fn light_black() -> String {
    "light black".into()
}

impl Default for Colors {
    fn default() -> Self {
        Colors {
            reached: cyan(),
            todo: magenta(),
            inactive: light_black(),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub look: Characters,

    #[serde(default)]
    pub colors: Colors,
}

impl AppConfig {
    pub fn reached_color(&self) -> Color {
        Color::parse(&self.colors.reached).unwrap_or(Color::Dark(BaseColor::Cyan))
    }
    pub fn todo_color(&self) -> Color {
        Color::parse(&self.colors.todo).unwrap_or(Color::Dark(BaseColor::Magenta))
    }
    pub fn inactive_color(&self) -> Color {
        Color::parse(&self.colors.inactive).unwrap_or(Color::Light(BaseColor::Black))
    }
}

/// Load the user config, falling back to defaults on any problem.
///
/// Config is non-essential and accessed during rendering, so a missing,
/// unreadable, or malformed file must never crash the app — we just use the
/// built-in defaults. A missing file is seeded with the defaults (best effort).
pub fn load_configuration_file() -> AppConfig {
    let Ok(cf) = config_file() else {
        return AppConfig::default();
    };

    match File::open(&cf) {
        Ok(ref mut f) => {
            let mut j = String::new();
            if f.read_to_string(&mut j).is_err() {
                return AppConfig::default();
            }
            // A malformed config falls back to defaults rather than crashing.
            toml::from_str(&j).unwrap_or_default()
        }
        Err(_) => {
            // No config yet: seed the file with defaults (best effort).
            if let Ok(dc) = toml::to_string(&AppConfig::default())
                && let Ok(ref mut file) = OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(&cf)
            {
                let _ = file.write_all(dc.as_bytes());
            }
            AppConfig::default()
        }
    }
}

fn project_dirs() -> Result<ProjectDirs, String> {
    ProjectDirs::from("rs", "nerdypepper", "dijo")
        .ok_or_else(|| "could not determine a home directory".to_string())
}

pub fn config_file() -> Result<PathBuf, String> {
    let proj_dirs = project_dirs()?;
    let dir = proj_dirs.config_dir();
    fs::create_dir_all(dir).map_err(|e| format!("could not create config dir: {e}"))?;
    Ok(dir.join("config.toml"))
}

pub fn habit_file() -> Result<PathBuf, String> {
    let proj_dirs = project_dirs()?;
    let dir = proj_dirs.data_dir();
    fs::create_dir_all(dir).map_err(|e| format!("could not create data dir: {e}"))?;
    Ok(dir.join("habit_record.json"))
}

pub fn archive_dir() -> Result<PathBuf, String> {
    let proj_dirs = project_dirs()?;
    let archive_path = proj_dirs.data_dir().join("archive");
    fs::create_dir_all(&archive_path).map_err(|e| format!("could not create archive dir: {e}"))?;
    Ok(archive_path)
}

/// Scan archive files and return reached-goal dates grouped by habit name.
/// Archive files are `{month}_{year}.json` containing arrays of serialized habits.
/// Each habit has "name", "goal", and "stats" fields.
pub fn load_archived_reached_goals() -> HashMap<String, HashSet<NaiveDate>> {
    let mut result: HashMap<String, HashSet<NaiveDate>> = HashMap::new();
    let archive_path = match archive_dir() {
        Ok(p) => p,
        Err(_) => return result,
    };

    let entries = match fs::read_dir(&archive_path) {
        Ok(e) => e,
        Err(_) => return result,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let mut f = match File::open(&path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut contents = String::new();
        if f.read_to_string(&mut contents).is_err() {
            continue;
        }

        let habits: Vec<serde_json::Value> = match serde_json::from_str(&contents) {
            Ok(v) => v,
            Err(_) => continue,
        };

        for habit in habits {
            let name = match habit.get("name").and_then(|n| n.as_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let dates = result.entry(name).or_default();
            dates.extend(archived_reached_dates(&habit));
        }
    }

    result
}

/// The reached-goal dates recorded in one archived habit's JSON blob.
///
/// Pure so it can be unit-tested without touching the filesystem. Daily habits
/// judge each day against the goal; weekly `Count`/`Float` habits aggregate
/// their recorded days into Mon–Sun weeks (within this one archive month) and
/// mark every recorded day of a week that met its goal. A goal of `0` means
/// "just track it", so every recorded day/week counts.
fn archived_reached_dates(habit: &serde_json::Value) -> Vec<NaiveDate> {
    let habit_type = habit.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let goal = habit.get("goal");
    let period = habit
        .get("period")
        .and_then(|p| p.as_str())
        .unwrap_or("Daily");
    let stats = match habit.get("stats").and_then(|s| s.as_object()) {
        Some(s) => s,
        None => return Vec::new(),
    };

    // Numeric goal and per-day contribution, shared by the daily and weekly
    // branches. Bit habits carry no numeric goal and are always daily.
    let numeric_goal: u64 = match habit_type {
        "Count" => goal.and_then(|g| g.as_u64()).unwrap_or(0),
        "Float" => goal
            .and_then(|g| g.as_object())
            .and_then(|o| o.get("value"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        _ => 0,
    };
    let day_value = |value: &serde_json::Value| -> u64 {
        match habit_type {
            "Count" => value.as_u64().unwrap_or(0),
            "Float" => value
                .as_object()
                .and_then(|o| o.get("value"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            _ => 0,
        }
    };

    let parse = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
    let mut out = Vec::new();

    if period == "Weekly" && (habit_type == "Count" || habit_type == "Float") {
        let mut weeks: HashMap<NaiveDate, (u64, Vec<NaiveDate>)> = HashMap::new();
        for (date_str, value) in stats {
            if let Some(date) = parse(date_str) {
                let entry = weeks.entry(week_bounds(date).0).or_default();
                entry.0 += day_value(value);
                entry.1.push(date);
            }
        }
        for (_monday, (sum, days)) in weeks {
            if numeric_goal == 0 || sum >= numeric_goal {
                out.extend(days);
            }
        }
    } else {
        for (date_str, value) in stats {
            let Some(date) = parse(date_str) else {
                continue;
            };
            let reached = match habit_type {
                "Bit" => value.as_bool().unwrap_or(false),
                "Count" | "Float" => numeric_goal == 0 || day_value(value) >= numeric_goal,
                _ => false,
            };
            if reached {
                out.push(date);
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, 1, day).unwrap()
    }

    fn reached(habit: serde_json::Value) -> HashSet<NaiveDate> {
        archived_reached_dates(&habit).into_iter().collect()
    }

    #[test]
    fn week_bounds_wraps_a_monday_to_sunday_span() {
        // 2024-01-03 is a Wednesday.
        assert_eq!(week_bounds(d(3)), (d(1), d(7)));
    }

    #[test]
    fn daily_count_marks_days_that_meet_the_goal() {
        let got = reached(json!({
            "type": "Count", "name": "water", "goal": 3,
            "stats": { "2024-01-01": 2, "2024-01-02": 3, "2024-01-03": 5 },
        }));
        assert_eq!(got, HashSet::from([d(2), d(3)]));
    }

    #[test]
    fn daily_bit_marks_true_days() {
        let got = reached(json!({
            "type": "Bit", "name": "read",
            "stats": { "2024-01-01": true, "2024-01-02": false },
        }));
        assert_eq!(got, HashSet::from([d(1)]));
    }

    #[test]
    fn weekly_count_marks_the_days_of_a_reached_week_only() {
        let got = reached(json!({
            "type": "Count", "name": "gym", "goal": 3, "period": "Weekly",
            "stats": {
                // week of Jan 01–07 sums to 3 -> reached
                "2024-01-01": 1, "2024-01-03": 1, "2024-01-05": 1,
                // week of Jan 08–14 sums to 2 -> not reached
                "2024-01-08": 2,
            },
        }));
        assert_eq!(got, HashSet::from([d(1), d(3), d(5)]));
    }

    #[test]
    fn a_zero_goal_counts_every_recorded_day() {
        let got = reached(json!({
            "type": "Count", "name": "log", "goal": 0,
            "stats": { "2024-01-01": 0, "2024-01-02": 1 },
        }));
        assert_eq!(got, HashSet::from([d(1), d(2)]));
    }
}
