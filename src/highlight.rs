use crate::errors::Error;
use once_cell::sync::Lazy;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use syntect::highlighting::ThemeSet;
use syntect::html::{css_for_theme_with_class_style, line_tokens_to_classed_spans, ClassStyle};
use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};
use syntect::util::LinesWithEndings;

static LIGHT_CSS: Lazy<String> = Lazy::new(|| {
    let data = include_str!("themes/ayu-light.tmTheme");
    let theme = ThemeSet::load_from_reader(&mut Cursor::new(data)).expect("loading theme");
    css_for_theme_with_class_style(&theme, ClassStyle::Spaced).expect("generating CSS")
});

static DARK_CSS: Lazy<String> = Lazy::new(|| {
    let data = include_str!("themes/ayu-dark.tmTheme");
    let theme = ThemeSet::load_from_reader(&mut Cursor::new(data)).expect("loading theme");
    css_for_theme_with_class_style(&theme, ClassStyle::Spaced).expect("generating CSS")
});

pub static DATA: Lazy<Data> = Lazy::new(|| {
    let style = Css::new("style", include_str!("themes/style.css"));
    let light = Css::new("light", &LIGHT_CSS);
    let dark = Css::new("dark", &DARK_CSS);

    Data {
        style,
        dark,
        light,
        syntax_set: SyntaxSet::load_defaults_newlines(),
    }
});

/// Combines CSS content with a filename containing the hash of the content.
pub struct Css<'a> {
    pub name: String,
    pub content: &'a str,
}

pub struct Data<'a> {
    pub style: Css<'a>,
    pub light: Css<'a>,
    pub dark: Css<'a>,
    pub syntax_set: SyntaxSet,
}

impl<'a> Css<'a> {
    fn new(name: &str, content: &'a str) -> Self {
        let name = format!(
            "/{name}.{}.css",
            hex::encode(Sha256::digest(content.as_bytes()))
                .get(0..16)
                .expect("at least 16 characters")
        );

        Self { name, content }
    }
}

pub fn highlight(source: &str, ext: &str) -> Result<String, Error> {
    let syntax_ref = DATA
        .syntax_set
        .find_syntax_by_extension(ext)
        .unwrap_or_else(|| DATA.syntax_set.find_syntax_by_extension("txt").unwrap());

    let mut parse_state = ParseState::new(syntax_ref);
    let mut html = String::from("<table><tbody>");
    let mut scope_stack = ScopeStack::new();

    for (mut line_number, line) in LinesWithEndings::from(source).enumerate() {
        let parsed = parse_state.parse_line(line, &DATA.syntax_set)?;
        let (formatted, delta) = line_tokens_to_classed_spans(
            line,
            parsed.as_slice(),
            ClassStyle::Spaced,
            &mut scope_stack,
        )?;

        line_number += 1;
        let formatted_str = formatted.as_str();
        let line_number =
            format!(r#"<td class="line-number"><a href=#L{line_number}>{line_number:>4}</a></td>"#);
        html.push_str(&line_number);

        let line = format!(r#"<td class="line">{formatted_str}"#);
        html.push_str(&line);
        html.push_str(&"</span>".repeat(delta.max(0).try_into()?));
        html.push_str("</td></tr>");
    }

    html.push_str("</tbody></table>");

    Ok(html)
}
