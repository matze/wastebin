use crate::srv::Entry;
use crate::Error;
use axum::http::header;
use axum::response::IntoResponse;
use once_cell::sync::Lazy;
use std::io::Cursor;
use syntect::highlighting::ThemeSet;
use syntect::html::{css_for_theme_with_class_style, line_tokens_to_classed_spans, ClassStyle};
use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};
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

        let mut parse_state = ParseState::new(syntax_ref);
        let mut open_spans = 0;
        let mut html = String::new();
        let mut scope_stack = ScopeStack::new();

        for (line_number, line) in LinesWithEndings::from(&entry.text).enumerate() {
            let parsed = parse_state.parse_line(line, &self.syntax_set).unwrap();
            let (formatted, delta) = line_tokens_to_classed_spans(
                line,
                parsed.as_slice(),
                ClassStyle::Spaced,
                &mut scope_stack,
            )
            .unwrap();

            open_spans += delta;
            let anchor = format!(
                r#"<span class="line-number"><a href=#{line_number}>{line_number:>4}</a></span>"#
            );
            html.push_str(&anchor);
            html.push_str(formatted.as_str());
        }

        html.push_str(&"</span>".repeat(open_spans.max(0) as usize));
        Ok(html)
    }
}
