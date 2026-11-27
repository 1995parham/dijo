mod app;
mod command;
mod habit;
mod stats;
mod theme;
mod utils;
mod views;

use crate::app::App;
use crate::command::open_command_window;
use crate::utils::{AppConfig, load_configuration_file};

use chrono::{Local, NaiveDate};
use clap::{Arg, Command as ClapApp};

use cursive::views::{LinearLayout, NamedView};
use cursive::{Cursive, CursiveExt};
use std::sync::LazyLock;

/// The version that `dijo --version` prints. A release build takes it from the
/// tag through `DIJO_VERSION`, so a tag alone makes a release; any other build
/// uses the version in `Cargo.toml`.
const VERSION: &str = match option_env!("DIJO_VERSION") {
    Some(v) => v,
    None => clap::crate_version!(),
};

pub static CONFIGURATION: LazyLock<AppConfig> = LazyLock::new(load_configuration_file);

fn main() {
    let matches = ClapApp::new(clap::crate_name!())
        .version(VERSION)
        .author(clap::crate_authors!("\n"))
        .about(clap::crate_description!())
        .help_template(
            "\
{before-help}{name} {version}
{author-with-newline}{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}
",
        )
        .arg(
            Arg::new("list")
                .long("list")
                .short('l')
                .action(clap::ArgAction::SetTrue)
                .help("list dijo habits")
                .conflicts_with("missing"),
        )
        .arg(
            Arg::new("missing")
                .long("missing")
                .short('m')
                .action(clap::ArgAction::Set)
                .value_name("HABIT")
                .help("print the days of this month that have no entry for HABIT")
                .conflicts_with("list"),
        )
        .subcommand(
            ClapApp::new("track")
                .about("add to a habit's entry, then save (do not run while the TUI is open)")
                .arg(Arg::new("habit").required(true).help("the habit's name"))
                .arg(
                    Arg::new("amount")
                        .default_value("1")
                        .allow_negative_numbers(true)
                        .help("how much to add; negative subtracts, a yes/no habit takes 1 or -1"),
                )
                .arg(
                    Arg::new("date")
                        .long("date")
                        .short('d')
                        .value_name("YYYY-MM-DD")
                        .value_parser(clap::value_parser!(NaiveDate))
                        .help("the day to change (default: today)"),
                ),
        )
        .subcommand(
            ClapApp::new("status")
                .about("print today's progress, such as `3/5 done today`")
                .arg(
                    Arg::new("json")
                        .long("json")
                        .action(clap::ArgAction::SetTrue)
                        .help("print JSON for scripts and a waybar custom module"),
                ),
        )
        .get_matches();

    let load_state = || {
        App::load_state().unwrap_or_else(|e| {
            eprintln!("dijo: {e}");
            std::process::exit(1);
        })
    };

    let fail = |e: String| -> ! {
        eprintln!("dijo: {e}");
        std::process::exit(1);
    };

    if let Some(track) = matches.subcommand_matches("track") {
        let mut app = load_state();
        let habit = track.get_one::<String>("habit").expect("required");
        let amount = track.get_one::<String>("amount").expect("defaulted");
        let date = track
            .get_one::<NaiveDate>("date")
            .copied()
            .unwrap_or_else(|| Local::now().date_naive());
        app.track(habit, amount, date).unwrap_or_else(|e| fail(e));
        app.save_state().unwrap_or_else(|e| fail(e));
    } else if let Some(status) = matches.subcommand_matches("status") {
        println!("{}", load_state().status_report(status.get_flag("json")));
    } else if matches.get_flag("list") {
        for h in load_state().list_habits() {
            println!("{h}");
        }
    } else if let Some(habit) = matches.get_one::<String>("missing") {
        let Some(days) = load_state().missed_habits_by_name(habit) else {
            eprintln!("dijo: habit `{habit}` does not exist");
            std::process::exit(1);
        };
        println!("forgot to fill {habit} on:\n");
        for day in days {
            println!("{day}");
        }
    } else {
        let mut s = Cursive::new();

        let app = load_state();
        let layout = NamedView::new(
            "Frame",
            LinearLayout::vertical().child(NamedView::new("Main", app)),
        );
        s.add_layer(layout);
        s.add_global_callback(':', open_command_window);

        s.set_theme(theme::theme_gen());
        s.run();

        let save_result = s.call_on_name("Main", |app: &mut App| app.save_state());
        if let Some(Err(e)) = save_result {
            eprintln!("dijo: {e}");
            std::process::exit(1);
        }
    }
}
