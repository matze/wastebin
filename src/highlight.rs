use crate::srv::Entry;
use crate::Error;
use axum::http::header;
use axum::response::IntoResponse;
use once_cell::sync::Lazy;
use std::io::Cursor;
use syntect::highlighting::ThemeSet;
use syntect::html::{css_for_theme_with_class_style, ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

pub static DATA: Lazy<Data> = Lazy::new(|| {
    let data = include_str!("themes/ayu-light.tmTheme");
    let light_theme = ThemeSet::load_from_reader(&mut Cursor::new(data)).unwrap();

    let data = include_str!("themes/ayu-dark.tmTheme");
    let dark_theme = ThemeSet::load_from_reader(&mut Cursor::new(data)).unwrap();

    Data {
        main: include_str!("themes/style.css"),
        light: css_for_theme_with_class_style(&light_theme, ClassStyle::Spaced).unwrap(),
        dark: css_for_theme_with_class_style(&dark_theme, ClassStyle::Spaced).unwrap(),
        syntax_set: SyntaxSet::load_defaults_newlines(),
    }
});

pub struct Data<'a> {
    pub main: &'a str,
    pub dark: String,
    pub light: String,
    pub syntax_set: SyntaxSet,
}

fn common_headers() -> [(header::HeaderName, &'static str); 2] {
    [
        (header::CONTENT_TYPE, "text/css"),
        (header::CACHE_CONTROL, "max-age=3600"),
    ]
}

impl<'a> Data<'a> {
    pub async fn main(&self) -> impl IntoResponse {
        (common_headers(), DATA.main.to_string())
    }

    pub async fn dark(&self) -> impl IntoResponse {
        (common_headers(), DATA.dark.clone())
    }

    pub async fn light(&self) -> impl IntoResponse {
        (common_headers(), DATA.light.clone())
    }

    pub fn highlight(&self, entry: Entry, ext: Option<String>) -> Result<String, Error> {
        let syntax_ref = match ext {
            Some(ext) => self
                .syntax_set
                .find_syntax_by_extension(&ext)
                .unwrap_or_else(|| DATA.syntax_set.find_syntax_by_extension("txt").unwrap()),
            None => DATA.syntax_set.find_syntax_by_extension("txt").unwrap(),
        };

        let mut generator = ClassedHTMLGenerator::new_with_class_style(
            syntax_ref,
            &self.syntax_set,
            ClassStyle::Spaced,
        );

        for line in LinesWithEndings::from(&entry.text) {
            generator
                .parse_html_for_line_which_includes_newline(line)
                .unwrap();
        }

        Ok(generator.finalize())
    }
}
