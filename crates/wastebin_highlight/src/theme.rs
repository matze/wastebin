use std::io::Cursor;
use std::str::FromStr;

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

/// An error which can be returned when parsing a [`Theme`] from its string representation.
#[derive(thiserror::Error, Debug)]
#[error("failed to parse theme name")]
pub struct ParseThemeNameError;

impl FromStr for Theme {
    type Err = ParseThemeNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ayu" => Ok(Theme::Ayu),
            "base16ocean" => Ok(Theme::Base16Ocean),
            "catppuccin" => Ok(Theme::Catppuccin),
            "coldark" => Ok(Theme::Coldark),
            "gruvbox" => Ok(Theme::Gruvbox),
            "monokai" => Ok(Theme::Monokai),
            "onehalf" => Ok(Theme::Onehalf),
            "solarized" => Ok(Theme::Solarized),
            _ => Err(ParseThemeNameError),
        }
    }
}

impl Theme {
    /// Generate combined light CSS for the given Theme.
    pub fn light_css(&self) -> Vec<u8> {
        combined_css("light", &self.light_theme())
    }

    /// Return light syntect highlighting theme.
    pub fn light_theme(&self) -> syntect::highlighting::Theme {
        let theme_set = two_face::theme::extra();

        match self {
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
        }
    }

    /// Generate combined dark CSS for the given Theme.
    pub fn dark_css(&self) -> Vec<u8> {
        combined_css("dark", &self.dark_theme())
    }

    /// Return dark syntect highlighting theme.
    pub fn dark_theme(&self) -> syntect::highlighting::Theme {
        let theme_set = two_face::theme::extra();

        match self {
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
        }
    }

    /// Return string representation of the theme name.
    pub fn name(&self) -> &'static str {
        // Make sure that these match the ones in the `FromStr` implementation.
        match self {
            Theme::Ayu => "ayu",
            Theme::Base16Ocean => "base16ocean",
            Theme::Catppuccin => "catppuccin",
            Theme::Coldark => "coldark",
            Theme::Gruvbox => "gruvbox",
            Theme::Monokai => "monokai",
            Theme::Onehalf => "onehalf",
            Theme::Solarized => "solarized",
        }
    }
}

/// Generate the highlighting colors for `theme` and add main foreground and background colors
/// based on the theme.
fn combined_css(color_scheme: &str, theme: &highlighting::Theme) -> Vec<u8> {
    let fg = theme.settings.foreground.expect("existing color");
    let bg = theme.settings.background.expect("existing color");

    format!(
        "{} {}",
        format_args!(
            ":root {{
      color-scheme: {color_scheme};
      --main-bg-color: rgb({}, {}, {}, {});
      --main-fg-color: rgb({}, {}, {}, {});
    }}",
            bg.r, bg.g, bg.b, bg.a, fg.r, fg.g, fg.b, fg.a
        ),
        css_for_theme_with_class_style(theme, ClassStyle::Spaced).expect("generating CSS")
    )
    .into_bytes()
}
