use askama::Template;
use axum::response::{IntoResponse, Response};
use axum_extra::{headers, TypedHeader};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::time::Duration;
use syntect::highlighting::{Color, ThemeSet};
use syntect::html::{css_for_theme_with_class_style, ClassStyle};
use two_face::theme::EmbeddedThemeName;

use crate::highlight::Theme;

/// An asset associated with a MIME type.
#[derive(Clone)]
pub struct Asset {
    /// Route that this will be served under.
    pub route: String,
    /// MIME type of this asset determined for the `ContentType` response header.
    mime: mime::Mime,
    /// Actual asset content.
    content: Vec<u8>,
}

/// Asset kind.
#[derive(Copy, Clone)]
pub enum Kind {
    Css,
    Js,
}

impl IntoResponse for Asset {
    fn into_response(self) -> Response {
        let content_type_header = headers::ContentType::from(self.mime);

        let headers = (
            TypedHeader(content_type_header),
            TypedHeader(headers::CacheControl::new().with_max_age(Duration::from_secs(100))),
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
pub struct Css {
    /// Main UI CSS stylesheet.
    pub style: Asset,
    /// Light theme colors.
    pub light: Asset,
    /// Dark theme colors.
    pub dark: Asset,
}

impl Css {
    /// Create CSS assets for `theme`.
    pub fn new(theme: Theme) -> Self {
        #[derive(Template)]
        #[template(path = "style.css", escape = "none")]
        struct StyleCss {
            light_background: Color,
            light_foreground: Color,
            dark_background: Color,
            dark_foreground: Color,
            light_asset: Asset,
            dark_asset: Asset,
        }

        let light_theme = light_theme(theme);
        let dark_theme = dark_theme(theme);

        // SAFETY: all supported color themes have a defined foreground and background color.
        let light_foreground = light_theme.settings.foreground.expect("existing color");
        let light_background = light_theme.settings.background.expect("existing color");
        let dark_foreground = dark_theme.settings.foreground.expect("existing color");
        let dark_background = dark_theme.settings.background.expect("existing color");

        let light = Asset::new_hashed(
            "light",
            Kind::Css,
            css_for_theme_with_class_style(&light_theme, ClassStyle::Spaced)
                .expect("generating CSS")
                .into_bytes(),
        );

        let dark = Asset::new_hashed(
            "dark",
            Kind::Css,
            css_for_theme_with_class_style(&dark_theme, ClassStyle::Spaced)
                .expect("generating CSS")
                .into_bytes(),
        );

        let style = StyleCss {
            light_background,
            light_foreground,
            dark_background,
            dark_foreground,
            light_asset: light.clone(),
            dark_asset: dark.clone(),
        };

        let style = Asset::new_hashed(
            "style",
            Kind::Css,
            style.render().expect("rendering style css").into_bytes(),
        );

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
}
