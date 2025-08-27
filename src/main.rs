#![allow(unused_must_use)]

mod app;
mod command;
mod habit;
mod theme;
mod utils;
mod views;

use crate::app::App;
use crate::command::open_command_window;
use crate::utils::{AppConfig, load_configuration_file};

use clap::{Arg, Command as ClapApp};

#[cfg(any(feature = "termion-backend", feature = "default"))]
use cursive::termion;

use cursive::views::{LinearLayout, NamedView};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref CONFIGURATION: AppConfig = load_configuration_file();
}

fn main() {
    let matches = ClapApp::new(clap::crate_name!())
        .version(clap::crate_version!())
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
                .help("missings habits")
                .conflicts_with("list"),
        )
        .get_matches();

    if matches.get_flag("list") {
        for h in App::load_state().list_habits() {
            println!("{h}");
        }
    } else if let Some(habit) = matches.get_one::<String>("missing") {
        println!("forgot to fill {habit} on:\n");
        for h in App::load_state().missed_habits_by_name(habit) {
            println!("{h}");
        }
    } else {
        #[cfg(any(feature = "termion-backend", feature = "default"))]
        let mut s = termion();

        let app = App::load_state();
        let layout = NamedView::new(
            "Frame",
            LinearLayout::vertical().child(NamedView::new("Main", app)),
        );
        s.add_layer(layout);
        s.add_global_callback(':', open_command_window);

        s.set_theme(theme::theme_gen());
        s.run();

        s.call_on_name("Main", |app: &mut App| app.save_state());
    }
}
