use std::sync::LazyLock;

use cursive::theme::Color::{self, *};
use cursive::theme::PaletteColor::*;
use cursive::theme::{BaseColor, BorderStyle, Palette, Theme};

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
        borders: BorderStyle::Simple,
        palette: pallete_gen(),
    }
}

/// Whether the terminal announces 24-bit color support.
static TRUECOLOR: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("COLORTERM").is_ok_and(|v| v.contains("truecolor") || v.contains("24bit"))
});

/// A color the terminal can show: an RGB color stays as it is on a truecolor
/// terminal and becomes the nearest bright base color anywhere else.
pub fn displayable(color: Color) -> Color {
    match color {
        Rgb(r, g, b) if !*TRUECOLOR => nearest_base(r, g, b),
        c => c,
    }
}

fn nearest_base(r: u8, g: u8, b: u8) -> Color {
    const BASES: [(BaseColor, [i32; 3]); 6] = [
        (BaseColor::Red, [255, 85, 85]),
        (BaseColor::Green, [85, 255, 85]),
        (BaseColor::Yellow, [255, 255, 85]),
        (BaseColor::Blue, [85, 85, 255]),
        (BaseColor::Magenta, [255, 85, 255]),
        (BaseColor::Cyan, [85, 255, 255]),
    ];
    let rgb = [i32::from(r), i32::from(g), i32::from(b)];
    let (lo, hi) = (rgb.iter().min().unwrap(), rgb.iter().max().unwrap());
    if hi == lo {
        return Light(BaseColor::White); // a grey has no hue
    }
    // Stretch the channels to the range of the base colors first, so a pastel
    // keeps its hue instead of landing on white.
    let stretched = rgb.map(|c| 85 + (c - lo) * 170 / (hi - lo));
    let distance = |c: &[i32; 3]| -> i32 { (0..3).map(|i| (c[i] - stretched[i]).pow(2)).sum() };
    BASES
        .iter()
        .min_by_key(|(_, c)| distance(c))
        .map_or(Light(BaseColor::White), |(base, _)| Light(*base))
}

/// The color of a period that is `ratio` (0..=1) of the way to its goal: the
/// accent itself when reached, a darker shade of it for partial progress. A
/// color that is not RGB has no shades, so partial progress gets `partial`.
pub fn progress_color(accent: Color, partial: Color, ratio: f64) -> Color {
    if ratio >= 1.0 {
        return accent;
    }
    match accent {
        Rgb(r, g, b) => {
            let factor = 0.35 + 0.4 * ratio.clamp(0.0, 1.0);
            let dim = |c: u8| (f64::from(c) * factor).round() as u8;
            Rgb(dim(r), dim(g), dim(b))
        }
        _ => partial,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_base_keeps_the_hue() {
        assert_eq!(nearest_base(0xa6, 0xe3, 0xa1), Light(BaseColor::Green));
        assert_eq!(nearest_base(0x89, 0xb4, 0xfa), Light(BaseColor::Blue));
        assert_eq!(nearest_base(0xf3, 0x8b, 0xa8), Light(BaseColor::Red));
    }

    #[test]
    fn progress_color_shades_an_rgb_accent() {
        let accent = Rgb(200, 100, 0);
        let partial = Light(BaseColor::Yellow);
        assert_eq!(progress_color(accent, partial, 1.0), accent);
        let Rgb(half, ..) = progress_color(accent, partial, 0.5) else {
            panic!("an RGB accent keeps RGB shades");
        };
        let Rgb(low, ..) = progress_color(accent, partial, 0.1) else {
            panic!("an RGB accent keeps RGB shades");
        };
        assert!(low < half && half < 200);
    }

    #[test]
    fn progress_color_falls_back_for_named_colors() {
        let accent = Dark(BaseColor::Cyan);
        let partial = Light(BaseColor::Yellow);
        assert_eq!(progress_color(accent, partial, 0.5), partial);
        assert_eq!(progress_color(accent, partial, 1.0), accent);
    }
}
