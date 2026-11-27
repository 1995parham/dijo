use chrono::{Datelike, Days, NaiveDate};
use cursive::theme::{BaseColor, Color};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Width of the area inside a habit card (31 fits the longest month in the
/// sparkline view, one column per day).
pub const VIEW_WIDTH: usize = 31;
/// Height of a habit card, borders included.
pub const VIEW_HEIGHT: usize = 10;
/// Width of a habit card, borders included.
pub const CARD_WIDTH: usize = VIEW_WIDTH + 2;
/// Blank columns between two cards.
pub const CARD_GAP: usize = 1;
/// The most cards in one row; a narrow terminal gets fewer.
pub const GRID_WIDTH: usize = 3;

/// Width of `cols` cards side by side.
pub fn grid_width(cols: usize) -> usize {
    cols * CARD_WIDTH + cols.saturating_sub(1) * CARD_GAP
}

/// Write `contents` to `path` through a sibling temp file and a rename, so a
/// crash mid-write leaves the old file intact instead of truncated.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("invalid file name: {}", path.display()))?;
    let tmp = path.with_file_name(format!("{name}.tmp"));
    let mut f =
        File::create(&tmp).map_err(|e| format!("could not open {name} for writing: {e}"))?;
    f.write_all(contents)
        .and_then(|()| f.sync_all())
        .map_err(|e| format!("could not write {name}: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("could not save {name}: {e}"))
}

/// The Monday and Sunday bounding the ISO-style week that contains `date`.
/// Used by weekly-goal habits to aggregate a week's entries.
pub fn week_bounds(date: NaiveDate) -> (NaiveDate, NaiveDate) {
    let offset = date.weekday().num_days_from_monday() as u64;
    let monday = date.checked_sub_days(Days::new(offset)).unwrap_or(date);
    let sunday = monday.checked_add_days(Days::new(6)).unwrap_or(monday);
    (monday, sunday)
}

/// The days of the month of `today`, before `today`, that have no entry.
///
/// A habit that has no entry before this month and nothing archived was made
/// this month, so the days before its first entry are not counted as missed.
pub fn missed_days(dates: &[NaiveDate], existed_before: bool, today: NaiveDate) -> Vec<NaiveDate> {
    let month_start = today.with_day(1).unwrap_or(today);
    let existed_before = existed_before || dates.iter().any(|&d| d < month_start);
    let start = if existed_before {
        month_start
    } else {
        match dates.iter().min() {
            Some(&first) => first.max(month_start),
            None => return Vec::new(),
        }
    };
    let recorded: HashSet<NaiveDate> = dates.iter().copied().collect();
    start
        .iter_days()
        .take_while(|&d| d < today)
        .filter(|d| !recorded.contains(d))
        .collect()
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
    /// One cell of the heatmap grids.
    #[serde(default = "heat_char")]
    pub heat_chr: char,
}

fn heat_char() -> char {
    '■'
}

fn base_char() -> char {
    '·'
}

impl Default for Characters {
    fn default() -> Self {
        Characters {
            true_chr: '●',
            false_chr: '✕',
            future_chr: '·',
            missing_chr: '○',
            heat_chr: heat_char(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Colors {
    /// Reached days, when `accents` is empty.
    #[serde(default = "reached")]
    pub reached: String,
    /// Days with some progress that did not reach the goal.
    #[serde(default = "todo")]
    pub todo: String,
    /// Days with an entry but no progress: a "no" or a count of zero.
    #[serde(default = "undone")]
    pub undone: String,
    /// Borders of unfocused cards, days with no entry, labels.
    #[serde(default = "inactive")]
    pub inactive: String,
    /// One color per habit, used in turn. An empty list gives every habit the
    /// `reached` color.
    #[serde(default = "accents")]
    pub accents: Vec<String>,
}

fn reached() -> String {
    "#a6e3a1".into()
}
fn todo() -> String {
    "#f9e2af".into()
}
fn undone() -> String {
    "#f38ba8".into()
}
fn inactive() -> String {
    "light black".into()
}
fn accents() -> Vec<String> {
    [
        "#89b4fa", "#a6e3a1", "#fab387", "#cba6f7", "#94e2d5", "#f5c2e7", "#f9e2af", "#f38ba8",
    ]
    .map(String::from)
    .to_vec()
}

impl Default for Colors {
    fn default() -> Self {
        Colors {
            reached: reached(),
            todo: todo(),
            undone: undone(),
            inactive: inactive(),
            accents: accents(),
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
        parse_color(&self.colors.reached, Color::Light(BaseColor::Green))
    }
    pub fn todo_color(&self) -> Color {
        parse_color(&self.colors.todo, Color::Light(BaseColor::Yellow))
    }
    pub fn undone_color(&self) -> Color {
        parse_color(&self.colors.undone, Color::Light(BaseColor::Red))
    }
    pub fn inactive_color(&self) -> Color {
        parse_color(&self.colors.inactive, Color::Light(BaseColor::Black))
    }
    /// The color of the habit at position `idx` in the grid.
    pub fn accent(&self, idx: usize) -> Color {
        let accents = &self.colors.accents;
        if accents.is_empty() {
            return self.reached_color();
        }
        parse_color(&accents[idx % accents.len()], self.reached_color())
    }
}

fn parse_color(value: &str, fallback: Color) -> Color {
    crate::theme::displayable(Color::parse(value).unwrap_or(fallback))
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

/// Merge the habits of a new archive run into the habits already stored in an
/// archive file of the same month. A habit present in both keeps its old
/// entries and takes the new ones (a new entry wins on the same day); its other
/// fields come from the new copy, which is the more recent one.
pub fn merge_archive(
    mut existing: Vec<serde_json::Value>,
    new: Vec<serde_json::Value>,
) -> Vec<serde_json::Value> {
    for mut habit in new {
        let name = habit.get("name").cloned();
        let Some(old) = existing
            .iter_mut()
            .find(|h| name.is_some() && h.get("name") == name.as_ref())
        else {
            existing.push(habit);
            continue;
        };
        if let Some(old_stats) = old.get("stats").and_then(|s| s.as_object()) {
            let mut stats = old_stats.clone();
            if let Some(new_stats) = habit.get("stats").and_then(|s| s.as_object()) {
                stats.extend(new_stats.clone());
            }
            habit["stats"] = serde_json::Value::Object(stats);
        }
        *old = habit;
    }
    existing
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
        "Count" | "Limit" => goal.and_then(|g| g.as_u64()).unwrap_or(0),
        "Float" => goal
            .and_then(|g| g.as_object())
            .and_then(|o| o.get("value"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        _ => 0,
    };
    let day_value = |value: &serde_json::Value| -> u64 {
        match habit_type {
            "Count" | "Limit" => value.as_u64().unwrap_or(0),
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
                "Limit" => day_value(value) <= numeric_goal,
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
    fn merge_archive_keeps_the_entries_of_an_earlier_run() {
        let existing = vec![
            json!({"type": "Count", "name": "gym", "goal": 1,
                   "stats": {"2024-01-01": 1, "2024-01-02": 1}}),
            json!({"type": "Count", "name": "gone", "goal": 1, "stats": {"2024-01-05": 1}}),
        ];
        let new = vec![
            json!({"type": "Count", "name": "gym", "goal": 2,
                   "stats": {"2024-01-02": 3, "2024-01-09": 1}}),
            json!({"type": "Count", "name": "new", "goal": 1, "stats": {"2024-01-03": 1}}),
        ];
        let merged = merge_archive(existing, new);
        assert_eq!(merged.len(), 3);
        assert_eq!(
            merged[0],
            json!({"type": "Count", "name": "gym", "goal": 2,
                   "stats": {"2024-01-01": 1, "2024-01-02": 3, "2024-01-09": 1}})
        );
        assert_eq!(merged[1]["name"], "gone");
        assert_eq!(merged[2]["name"], "new");
    }

    #[test]
    fn write_atomic_replaces_the_file_and_leaves_no_temp_file() {
        let dir = std::env::temp_dir().join(format!("dijo-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("record.json");
        write_atomic(&path, b"old").unwrap();
        write_atomic(&path, b"new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        assert!(!dir.join("record.json.tmp").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missed_days_starts_at_the_first_entry_of_a_new_habit() {
        // made on the 3rd, filled on the 3rd and the 5th, today is the 7th
        assert_eq!(missed_days(&[d(3), d(5)], false, d(7)), vec![d(4), d(6)]);
    }

    #[test]
    fn missed_days_covers_the_whole_month_for_an_older_habit() {
        assert_eq!(missed_days(&[d(3)], true, d(5)), vec![d(1), d(2), d(4)]);
        let december = NaiveDate::from_ymd_opt(2023, 12, 30).unwrap();
        assert_eq!(missed_days(&[december, d(2)], false, d(3)), vec![d(1)]);
    }

    #[test]
    fn missed_days_is_empty_for_a_habit_without_entries() {
        assert!(missed_days(&[], false, d(10)).is_empty());
        assert_eq!(missed_days(&[], true, d(3)), vec![d(1), d(2)]);
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
    fn limit_marks_days_at_or_under_the_limit() {
        let got = reached(json!({
            "type": "Limit", "name": "smoke", "goal": 2,
            "stats": { "2024-01-01": 0, "2024-01-02": 2, "2024-01-03": 3 },
        }));
        assert_eq!(got, HashSet::from([d(1), d(2)]));
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
