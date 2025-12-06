use std::io::Cursor;

use syntect::highlighting::{self, ThemeSet};
use syntect::html::{ClassStyle, css_for_theme_with_class_style};
use two_face::theme::EmbeddedThemeName;

/// Supported themes.
#[derive(Copy, Clone)]
pub enum Theme {
    Ayu,
    Base16Ocean,
    Catppuccin,
    Coldark,
    Gruvbox,
    Monokai,
    Onehalf,
    Solarized,
}

impl Theme {
    /// Generate combined light CSS for the given Theme.
    pub fn light_css(&self) -> Vec<u8> {
        let theme_set = two_face::theme::extra();

        let theme = match self {
            Theme::Ayu => {
                let theme = include_str!("../themes/ayu-light.tmTheme");
                ThemeSet::load_from_reader(&mut Cursor::new(theme)).expect("loading theme")
            }
            Theme::Base16Ocean => theme_set.get(EmbeddedThemeName::Base16OceanLight).clone(),
            Theme::Catppuccin => theme_set.get(EmbeddedThemeName::CatppuccinLatte).clone(),
            Theme::Coldark => theme_set.get(EmbeddedThemeName::ColdarkCold).clone(),
            Theme::Gruvbox => theme_set.get(EmbeddedThemeName::GruvboxLight).clone(),
            Theme::Monokai => theme_set
                .get(EmbeddedThemeName::MonokaiExtendedLight)
                .clone(),
            Theme::Onehalf => theme_set.get(EmbeddedThemeName::OneHalfLight).clone(),
            Theme::Solarized => theme_set.get(EmbeddedThemeName::SolarizedLight).clone(),
        };

        combined_css("light", &theme)
    }

    /// Generate combined dark CSS for the given Theme.
    pub fn dark_css(&self) -> Vec<u8> {
        let theme_set = two_face::theme::extra();

        let theme = match self {
            Theme::Ayu => {
                let theme = include_str!("../themes/ayu-dark.tmTheme");
                ThemeSet::load_from_reader(&mut Cursor::new(theme)).expect("loading theme")
            }
            Theme::Base16Ocean => theme_set.get(EmbeddedThemeName::Base16OceanDark).clone(),
            Theme::Catppuccin => theme_set.get(EmbeddedThemeName::CatppuccinMocha).clone(),
            Theme::Coldark => theme_set.get(EmbeddedThemeName::ColdarkDark).clone(),
            Theme::Gruvbox => theme_set.get(EmbeddedThemeName::GruvboxDark).clone(),
            Theme::Monokai => theme_set.get(EmbeddedThemeName::MonokaiExtended).clone(),
            Theme::Onehalf => theme_set.get(EmbeddedThemeName::OneHalfDark).clone(),
            Theme::Solarized => theme_set.get(EmbeddedThemeName::SolarizedDark).clone(),
        };

        combined_css("dark", &theme)
    }
}

/// Generate the highlighting colors for `theme` and add main foreground and background colors
/// based on the theme.
fn combined_css(color_scheme: &str, theme: &highlighting::Theme) -> Vec<u8> {
    let fg = theme.settings.foreground.expect("existing color");
    let bg = theme.settings.background.expect("existing color");

    let main_colors = format!(
        ":root {{
  color-scheme: {color_scheme};
  --main-bg-color: rgb({}, {}, {}, {});
  --main-fg-color: rgb({}, {}, {}, {});
}}",
        bg.r, bg.g, bg.b, bg.a, fg.r, fg.g, fg.b, fg.a
    );

    format!(
        "{main_colors} {}",
        css_for_theme_with_class_style(theme, ClassStyle::Spaced).expect("generating CSS")
    )
    .into_bytes()
}
