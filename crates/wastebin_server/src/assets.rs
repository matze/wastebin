use std::io::Cursor;
use std::time::Duration;

use axum::response::{IntoResponse, Response};
use axum_extra::{TypedHeader, headers};
use sha2::{Digest, Sha256};
use syntect::highlighting::{self, ThemeSet};
use syntect::html::{ClassStyle, css_for_theme_with_class_style};
use two_face::theme::EmbeddedThemeName;

use crate::highlight::Theme;

/// An asset associated with a MIME type.
#[derive(Clone)]
pub(crate) struct Asset {
    /// Route that this will be served under.
    pub route: String,
    /// MIME type of this asset determined for the `ContentType` response header.
    mime: mime::Mime,
    /// Actual asset content.
    content: Vec<u8>,
}

/// Asset kind.
#[derive(Copy, Clone)]
pub(crate) enum Kind {
    Css,
    Js,
}

impl IntoResponse for Asset {
    fn into_response(self) -> Response {
        let content_type_header = headers::ContentType::from(self.mime);

        let headers = (
            TypedHeader(content_type_header),
            TypedHeader(
                headers::CacheControl::new()
                    .with_max_age(Duration::from_secs(60 * 60 * 24 * 30))
                    .with_immutable(),
            ),
        );

        (headers, self.content).into_response()
    }
}

impl Asset {
    /// Construct new asset under the given `name`, `mime` type and `content`.
    pub fn new(name: &str, mime: mime::Mime, content: Vec<u8>) -> Self {
        Self {
            route: format!("/{name}"),
            mime,
            content,
        }
    }

    /// Construct new hashed asset under the given `name`, `kind` and `content`.
    pub fn new_hashed(name: &str, kind: Kind, content: Vec<u8>) -> Self {
        let (mime, ext) = match kind {
            Kind::Css => (mime::TEXT_CSS, "css"),
            Kind::Js => (mime::TEXT_JAVASCRIPT, "js"),
        };

        let route = format!(
            "/{name}.{}.{ext}",
            hex::encode(Sha256::digest(&content))
                .get(0..16)
                .expect("at least 16 characters")
        );

        Self {
            route,
            mime,
            content,
        }
    }

    pub fn route(&self) -> &str {
        &self.route
    }
}

/// Collection of light and dark CSS and main UI style CSS derived from them.
pub(crate) struct Css {
    /// Main UI CSS stylesheet.
    pub style: Asset,
    /// Light theme colors.
    pub light: Asset,
    /// Dark theme colors.
    pub dark: Asset,
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

impl Css {
    /// Create CSS assets for `theme`.
    pub fn new(theme: Theme) -> Self {
        let light_theme = light_theme(theme);
        let dark_theme = dark_theme(theme);
        let style = Asset::new_hashed("style", Kind::Css, include_str!("style.css").into());
        let light = Asset::new_hashed("light", Kind::Css, combined_css("light", &light_theme));
        let dark = Asset::new_hashed("dark", Kind::Css, combined_css("dark", &dark_theme));

        Self { style, light, dark }
    }
}

fn light_theme(theme: Theme) -> syntect::highlighting::Theme {
    let theme_set = two_face::theme::extra();

    match theme {
        Theme::Ayu => {
            let theme = include_str!("themes/ayu-light.tmTheme");
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

fn dark_theme(theme: Theme) -> syntect::highlighting::Theme {
    let theme_set = two_face::theme::extra();

    match theme {
        Theme::Ayu => {
            let theme = include_str!("themes/ayu-dark.tmTheme");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashed_asset() {
        let asset = Asset::new_hashed("style", Kind::Css, String::from("body {}").into_bytes());
        assert_eq!(asset.route, "/style.62368a1a29259b30.css");

        let asset = Asset::new_hashed("main", Kind::Js, String::from("1 + 1").into_bytes());
        assert_eq!(asset.route, "/main.72fce59447a01f48.js");
    }

    #[test]
    fn asset_response() {
        let asset = Asset::new(
            "foo.css",
            mime::TEXT_CSS,
            String::from("body {}").into_bytes(),
        );

        let response = asset.into_response();
        let headers = response.headers();

        assert_eq!(headers.get(http::header::CONTENT_TYPE).unwrap(), "text/css");
        assert_eq!(
            headers.get(http::header::CACHE_CONTROL).unwrap(),
            "immutable, max-age=2592000"
        );
    }
}
