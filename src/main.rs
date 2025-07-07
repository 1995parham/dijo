#![allow(unused_must_use)]

mod app;
mod command;
mod habit;
mod theme;
mod utils;
mod views;

use crate::app::App;
use crate::command::{open_command_window, Command};
use crate::utils::{load_configuration_file, AppConfig};

use clap::{Arg, Command as ClapApp};

#[cfg(any(feature = "termion-backend", feature = "default"))]
use cursive::termion;

#[cfg(feature = "crossterm-backend")]
use cursive::crossterm;

use cursive::views::{LinearLayout, NamedView};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref CONFIGURATION: AppConfig = load_configuration_file();
}

fn main() {
    let matches = ClapApp::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::new("command")
                .long("command")
                .short('c')
                .value_name("CMD")
                .help("run a dijo command"),
        )
        .arg(
            Arg::new("list")
                .long("list")
                .short('l')
                .action(clap::ArgAction::SetTrue)
                .help("list dijo habits")
                .conflicts_with("command"),
        )
        .get_matches();

    if let Some(c) = matches.get_one::<String>("command") {
        let command = Command::from_string(c);
        match command {
            Ok(Command::TrackUp(_)) | Ok(Command::TrackDown(_)) => {
                let mut app = App::load_state();
                app.parse_command(command);
                app.save_state();
            }
            Err(e) => {
                eprintln!("{e}");
            }
            _ => eprintln!(
                "Commands other than `track-up` and `track-down` are currently not supported!"
            ),
        }
    } else if matches.get_flag("list") {
        for h in App::load_state().list_habits() {
            println!("{h}");
        }
    } else {
        #[cfg(any(feature = "termion-backend", feature = "default"))]
        let mut s = termion();

        #[cfg(feature = "crossterm-backend")]
        let mut s = crossterm();

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
