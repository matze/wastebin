use crate::{Entry, Error};
use axum::response::{IntoResponse, IntoResponseParts};
use axum::{headers, TypedHeader};
use once_cell::sync::Lazy;
use std::io::Cursor;
use std::time::Duration;
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

fn common_headers() -> impl IntoResponseParts {
    (
        TypedHeader(headers::ContentType::from(mime::TEXT_CSS)),
        TypedHeader(headers::CacheControl::new().with_max_age(Duration::from_secs(3600))),
    )
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
        let mut html = String::from("<table><tbody>");
        let mut scope_stack = ScopeStack::new();

        for (mut line_number, line) in LinesWithEndings::from(&entry.text).enumerate() {
            let parsed = parse_state.parse_line(line, &self.syntax_set)?;
            let (formatted, delta) = line_tokens_to_classed_spans(
                line,
                parsed.as_slice(),
                ClassStyle::Spaced,
                &mut scope_stack,
            )
            .unwrap();

            line_number += 1;
            let formatted_str = formatted.as_str();
            let line_number = format!(
                r#"<td class="line-number"><a href=#L{line_number}>{line_number:>4}</a></td>"#
            );
            html.push_str(&line_number);

            let line = format!(r#"<td class="line">{formatted_str}"#);
            html.push_str(&line);
            html.push_str(&"</span>".repeat(delta.max(0) as usize));
            html.push_str("</td></tr>");
        }

        html.push_str("</tbody></table>");

        Ok(html)
    }
}
