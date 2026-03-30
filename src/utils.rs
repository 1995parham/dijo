use chrono::NaiveDate;
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

pub fn load_configuration_file() -> AppConfig {
    let cf = config_file();
    if let Ok(ref mut f) = File::open(&cf) {
        let mut j = String::new();
        f.read_to_string(&mut j)
            .unwrap_or_else(|e| panic!("Failed to read config file: `{e}`"));
        toml::from_str(&j).unwrap_or_else(|e| panic!("Invalid config file: `{e}`"))
    } else {
        if let Ok(dc) = toml::to_string(&AppConfig::default()) {
            match OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&cf)
            {
                Ok(ref mut file) => {
                    file.write_all(dc.as_bytes()).unwrap();
                }
                Err(_) => panic!("Unable to write config file to disk!"),
            };
        }
        Default::default()
    }
}

fn project_dirs() -> ProjectDirs {
    ProjectDirs::from("rs", "nerdypepper", "dijo")
        .unwrap_or_else(|| panic!("Invalid home directory!"))
}

pub fn config_file() -> PathBuf {
    let proj_dirs = project_dirs();
    let mut data_file = PathBuf::from(proj_dirs.config_dir());
    fs::create_dir_all(&data_file).unwrap_or_else(|e| panic!("Failed to create config dir: `{e}`"));
    data_file.push("config.toml");
    data_file
}

pub fn habit_file() -> PathBuf {
    let proj_dirs = project_dirs();
    let mut data_file = PathBuf::from(proj_dirs.data_dir());
    fs::create_dir_all(&data_file).unwrap_or_else(|e| panic!("Failed to create data dir: `{e}`"));
    data_file.push("habit_record.json");
    data_file
}

pub fn archive_dir() -> PathBuf {
    let proj_dirs = project_dirs();
    let mut archive_path = PathBuf::from(proj_dirs.data_dir());
    archive_path.push("archive");
    fs::create_dir_all(&archive_path)
        .unwrap_or_else(|e| panic!("Failed to create archive dir: `{e}`"));
    archive_path
}

/// Scan archive files and return reached-goal dates grouped by habit name.
/// Archive files are `{month}_{year}.json` containing arrays of serialized habits.
/// Each habit has "name", "goal", and "stats" fields.
pub fn load_archived_reached_goals() -> HashMap<String, HashSet<NaiveDate>> {
    let archive_path = archive_dir();
    let mut result: HashMap<String, HashSet<NaiveDate>> = HashMap::new();

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

            let goal = habit.get("goal");
            let habit_type = habit.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let stats = match habit.get("stats").and_then(|s| s.as_object()) {
                Some(s) => s,
                None => continue,
            };

            let dates = result.entry(name).or_default();

            for (date_str, value) in stats {
                let date = match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                    Ok(d) => d,
                    Err(_) => continue,
                };

                let reached = match habit_type {
                    "Bit" => value.as_bool().unwrap_or(false),
                    "Count" => {
                        let val = value.as_u64().unwrap_or(0) as u32;
                        let g = goal.and_then(|g| g.as_u64()).unwrap_or(0) as u32;
                        g == 0 || val >= g
                    }
                    "Float" => {
                        let val = value
                            .as_object()
                            .and_then(|o| o.get("value"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as u32;
                        let g = goal
                            .and_then(|g| g.as_object())
                            .and_then(|o| o.get("value"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as u32;
                        g == 0 || val >= g
                    }
                    _ => false,
                };

                if reached {
                    dates.insert(date);
                }
            }
        }
    }

    result
}
