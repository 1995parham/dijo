use std::fmt;
use std::str::FromStr;

use cursive::Cursive;
use cursive::event::{Event, EventResult, Key};
use cursive::theme::{BaseColor, Color, ColorStyle};
use cursive::view::Resizable;
use cursive::views::{Dialog, EditView, LinearLayout, OnEventView, TextView};

use crate::app::App;
use crate::habit::GoalPeriod;
use crate::utils::{GRID_WIDTH, VIEW_WIDTH};

static COMMANDS: &[&str] = &[
    "add",
    "describe",
    "delete",
    "month-prev",
    "month-next",
    "quit",
    "write",
    "help",
    "writeandquit",
    "archive",
    "dashboard",
];

fn get_command_completion(prefix: &str) -> Option<String> {
    let first_match = COMMANDS.iter().find(|&x| x.starts_with(prefix));
    first_match.map(|&x| x.into())
}

fn get_habit_completion(prefix: &str, habit_names: &[String]) -> Option<String> {
    let first_match = habit_names.iter().find(|&x| x.starts_with(prefix));
    first_match.map(|x| x.into())
}

pub fn open_command_window(s: &mut Cursive) {
    let habit_list: Vec<String> = s
        .call_on_name("Main", |view: &mut App| view.list_habits())
        .unwrap();
    let style = ColorStyle::new(Color::Dark(BaseColor::Black), Color::Dark(BaseColor::White));
    let command_window = OnEventView::new(
        EditView::new()
            .filler(" ")
            .on_submit(call_on_app)
            .style(style),
    )
    .on_event_inner(
        Event::Key(Key::Tab),
        move |view: &mut EditView, _: &Event| {
            let contents = view.get_content();
            if !contents.contains(" ") {
                let completion = get_command_completion(&contents);
                if let Some(c) = completion {
                    let cb = view.set_content(c);
                    return Some(EventResult::Consumed(Some(cb)));
                };
                None
            } else {
                let word = contents.split(' ').next_back().unwrap();
                let completion = get_habit_completion(word, &habit_list);
                if let Some(c) = completion {
                    let cb = view.set_content(format!("{contents}") + &c[word.len()..]);
                    return Some(EventResult::Consumed(Some(cb)));
                };
                None
            }
        },
    )
    .fixed_width(VIEW_WIDTH * GRID_WIDTH);
    s.call_on_name("Frame", |view: &mut LinearLayout| {
        let mut commandline = LinearLayout::horizontal()
            .child(TextView::new(":"))
            .child(command_window);
        let _ = commandline.set_focus_index(1);
        view.add_child(commandline);
        let _ = view.set_focus_index(1);
    });
}

fn call_on_app(s: &mut Cursive, input: &str) {
    // things to do after recieving the command
    // 1. parse the command
    // 2. clean existing command messages
    // 3. remove the command window
    // 4. handle quit command
    s.call_on_name("Main", |view: &mut App| {
        let cmd = input.parse();
        view.clear_message();
        view.parse_command(cmd);
    });
    s.call_on_name("Frame", |view: &mut LinearLayout| {
        let _ = view.set_focus_index(0);
        view.remove_child(view.get_focus_index());
    });

    // special command that requires access to
    // our main cursive object, has to be parsed again
    // here
    // TODO: fix this somehow
    match input.parse::<Command>() {
        Ok(Command::Quit) | Ok(Command::WriteAndQuit) => s.quit(),
        Ok(Command::Dashboard) => open_dashboard(s),
        _ => {}
    }
}

/// Open a full-screen dashboard overlay for the currently focused habit.
/// Dismissed with `q` or `Esc`.
pub fn open_dashboard(s: &mut Cursive) {
    let dashboard = s
        .call_on_name("Main", |view: &mut App| view.focused_dashboard())
        .flatten();
    let Some((name, body)) = dashboard else {
        return;
    };

    let dialog = Dialog::around(TextView::new(body))
        .title(name)
        .button("close", |s| {
            s.pop_layer();
        });
    let view = OnEventView::new(dialog)
        .on_event(Event::Key(Key::Esc), |s| {
            s.pop_layer();
        })
        .on_event('q', |s| {
            s.pop_layer();
        });
    s.add_layer(view);
}

/// Open a popup with the focused habit's full description, line breaks
/// intact. Dismissed with `q` or `Esc`.
pub fn open_description(s: &mut Cursive) {
    let description = s
        .call_on_name("Main", |view: &mut App| view.focused_description())
        .flatten();
    let Some((name, body)) = description else {
        return;
    };

    let dialog = Dialog::around(TextView::new(body))
        .title(name)
        .button("close", |s| {
            s.pop_layer();
        });
    let view = OnEventView::new(dialog)
        .on_event(Event::Key(Key::Esc), |s| {
            s.pop_layer();
        })
        .on_event('q', |s| {
            s.pop_layer();
        });
    s.add_layer(view);
}

#[derive(Debug, PartialEq)]
pub enum GoalKind {
    Count(u32),
    Bit,
    Float(u32, u8),
    Addiction(u32),
}

impl FromStr for GoalKind {
    type Err = CommandLineError;

    fn from_str(s: &str) -> Result<Self> {
        if let Some(n) = s.strip_prefix("<") {
            return n
                .parse::<u32>()
                .map_err(|_| CommandLineError::InvalidGoal(s.into()))
                .map(GoalKind::Addiction);
        } else if s.contains(".") {
            let value = s
                .chars()
                .filter(|x| x.is_ascii_digit())
                .collect::<String>()
                .parse::<u32>()
                .map_err(|_| CommandLineError::InvalidCommand(s.into()))?;
            let precision = s.chars().skip_while(|&x| x != '.').count() - 1;
            return Ok(GoalKind::Float(value, precision as u8));
        }
        if let Ok(v) = s.parse::<u32>() {
            if v == 1 {
                return Ok(GoalKind::Bit);
            } else {
                return Ok(GoalKind::Count(v));
            }
        }
        Err(CommandLineError::InvalidCommand(s.into()))
    }
}

/// Split a trailing period marker (`/week`, `/weekly`, `/w`) off a goal token,
/// returning the bare goal expression and the period it denotes.
fn split_period(token: &str) -> (&str, GoalPeriod) {
    for suffix in ["/week", "/weekly", "/w"] {
        if let Some(base) = token.strip_suffix(suffix) {
            return (base, GoalPeriod::Weekly);
        }
    }
    (token, GoalPeriod::Daily)
}

#[derive(Debug, PartialEq)]
pub enum Command {
    Add(String, Option<GoalKind>, GoalPeriod),
    Describe(String, String),
    MonthPrev,
    MonthNext,
    Delete(String),
    Help(Option<String>),
    Write,
    Quit,
    Blank,
    WriteAndQuit,
    Archive,
    Dashboard,
}

#[derive(Debug)]
pub enum CommandLineError {
    InvalidCommand(String),     // command name
    InvalidArg(u32),            // position
    NotEnoughArgs(String, u32), // command name, required no. of args
    InvalidGoal(String),        // goal expression
}

impl std::error::Error for CommandLineError {}

impl fmt::Display for CommandLineError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CommandLineError::InvalidCommand(s) => write!(f, "Invalid command: `{s}`"),
            CommandLineError::InvalidArg(p) => write!(f, "Invalid argument at position {p}"),
            CommandLineError::NotEnoughArgs(s, n) => {
                write!(f, "Command `{s}` requires atleast {n} argument(s)!")
            }
            CommandLineError::InvalidGoal(s) => write!(f, "Invalid goal expression: `{s}`"),
        }
    }
}

type Result<T> = std::result::Result<T, CommandLineError>;

impl FromStr for Command {
    type Err = CommandLineError;

    fn from_str(input: &str) -> Result<Command> {
        let mut strings: Vec<&str> = input.trim().split(' ').collect();
        if strings.is_empty() {
            return Ok(Command::Blank);
        }

        let first = strings.first().unwrap().to_string();
        let mut args: Vec<String> = strings.iter_mut().skip(1).map(|s| s.to_string()).collect();
        let mut _add = |first: String| {
            if args.is_empty() {
                return Err(CommandLineError::NotEnoughArgs(first, 1));
            }
            let (goal, period) = match args.get(1) {
                Some(raw) => {
                    let (base, period) = split_period(raw);
                    let mut kind = GoalKind::from_str(base)?;
                    // A weekly `1` is "once a week", a legitimate count target,
                    // not a daily yes/no habit.
                    if period == GoalPeriod::Weekly && kind == GoalKind::Bit {
                        kind = GoalKind::Count(1);
                    }
                    (Some(kind), period)
                }
                None => (None, GoalPeriod::Daily),
            };
            Ok(Command::Add(
                args.get_mut(0).unwrap().to_string(),
                goal,
                period,
            ))
        };

        match first.as_ref() {
            "add" | "a" => _add(first),
            "delete" | "d" => {
                if args.is_empty() {
                    return Err(CommandLineError::NotEnoughArgs(first, 1));
                }
                Ok(Command::Delete(args[0].to_string()))
            }
            "describe" | "desc" => {
                if args.len() < 2 {
                    return Err(CommandLineError::NotEnoughArgs(first, 2));
                }
                let name = args[0].to_string();
                // the command line is single-line, so `\n` is the only way to
                // put a line break in a description
                let description = args[1..].join(" ").replace("\\n", "\n");
                Ok(Command::Describe(name, description))
            }
            "h" | "?" | "help" => {
                if args.is_empty() {
                    return Ok(Command::Help(None));
                }
                Ok(Command::Help(Some(args[0].to_string())))
            }
            "mprev" | "month-prev" => Ok(Command::MonthPrev),
            "mnext" | "month-next" => Ok(Command::MonthNext),
            "wq" | "writeandquit" => Ok(Command::WriteAndQuit),
            "q" | "quit" => Ok(Command::Quit),
            "w" | "write" => Ok(Command::Write),
            "archive" => Ok(Command::Archive),
            "dashboard" | "dash" => Ok(Command::Dashboard),
            "" => Ok(Command::Blank),
            s => Err(CommandLineError::InvalidCommand(s.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_joins_multi_word_text() {
        let cmd = "describe read a good book every night".parse::<Command>();
        assert_eq!(
            cmd.unwrap(),
            Command::Describe("read".into(), "a good book every night".into())
        );
    }

    #[test]
    fn describe_alias_desc_works() {
        let cmd = "desc gym leg day".parse::<Command>();
        assert_eq!(
            cmd.unwrap(),
            Command::Describe("gym".into(), "leg day".into())
        );
    }

    #[test]
    fn describe_unescapes_newlines() {
        let cmd = r"describe gym go\nlift".parse::<Command>();
        assert_eq!(
            cmd.unwrap(),
            Command::Describe("gym".into(), "go\nlift".into())
        );
    }

    #[test]
    fn describe_requires_name_and_text() {
        assert!(matches!(
            "describe read".parse::<Command>(),
            Err(CommandLineError::NotEnoughArgs(_, 2))
        ));
    }

    #[test]
    fn add_defaults_to_a_daily_goal() {
        assert_eq!(
            "add gym 3".parse::<Command>().unwrap(),
            Command::Add("gym".into(), Some(GoalKind::Count(3)), GoalPeriod::Daily)
        );
    }

    #[test]
    fn add_parses_weekly_goal_suffix() {
        for token in ["3/week", "3/weekly", "3/w"] {
            assert_eq!(
                format!("add gym {token}").parse::<Command>().unwrap(),
                Command::Add("gym".into(), Some(GoalKind::Count(3)), GoalPeriod::Weekly),
                "token `{token}` should parse as a weekly count goal",
            );
        }
    }

    #[test]
    fn weekly_one_is_a_count_not_a_bit() {
        // `1/week` means "once a week" — a genuine count target, unlike a bare
        // daily `1` which is a yes/no Bit habit.
        assert_eq!(
            "add floss 1/week".parse::<Command>().unwrap(),
            Command::Add("floss".into(), Some(GoalKind::Count(1)), GoalPeriod::Weekly)
        );
        assert_eq!(
            "add floss 1".parse::<Command>().unwrap(),
            Command::Add("floss".into(), Some(GoalKind::Bit), GoalPeriod::Daily)
        );
    }

    #[test]
    fn add_parses_weekly_float_goal() {
        assert_eq!(
            "add run 10.5/week".parse::<Command>().unwrap(),
            Command::Add(
                "run".into(),
                Some(GoalKind::Float(105, 1)),
                GoalPeriod::Weekly
            )
        );
    }
}
