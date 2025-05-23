use cursive::theme::Color::{self, *};
use cursive::theme::PaletteColor::*;
use cursive::theme::{BorderStyle, Palette, Theme};

pub fn pallete_gen() -> Palette {
    let mut p = Palette::default();
    p[Background] = TerminalDefault;
    p[Shadow] = TerminalDefault;
    p[View] = TerminalDefault;
    p[Primary] = TerminalDefault;
    p[Secondary] = TerminalDefault;
    p[Tertiary] = TerminalDefault;
    p[TitlePrimary] = TerminalDefault;
    p[Highlight] = TerminalDefault;
    p[HighlightInactive] = TerminalDefault;

    p
}

pub fn theme_gen() -> Theme {
    Theme {
        shadow: false,
        borders: BorderStyle::None,
        palette: pallete_gen(),
    }
}

pub fn cursor_bg() -> Color {
    Light(cursive::theme::BaseColor::Black)
}
