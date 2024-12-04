use crate::db::read::Entry;
use crate::errors::Error;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::io::Cursor;
use std::sync::LazyLock;
use syntect::highlighting::ThemeSet;
use syntect::html::{css_for_theme_with_class_style, line_tokens_to_classed_spans, ClassStyle};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

const HIGHLIGHT_LINE_LENGTH_CUTOFF: usize = 2048;

#[derive(Clone)]
pub struct Html(String);

static LIGHT_CSS: LazyLock<String> = LazyLock::new(|| {
    let theme = include_str!("themes/ayu-light.tmTheme");
    let theme = ThemeSet::load_from_reader(&mut Cursor::new(theme)).expect("loading theme");
    css_for_theme_with_class_style(&theme, ClassStyle::Spaced).expect("generating CSS")
});

static DARK_CSS: LazyLock<String> = LazyLock::new(|| {
    let theme = include_str!("themes/ayu-dark.tmTheme");
    let theme = ThemeSet::load_from_reader(&mut Cursor::new(theme)).expect("loading theme");
    css_for_theme_with_class_style(&theme, ClassStyle::Spaced).expect("generating CSS")
});

pub static DATA: LazyLock<Data> = LazyLock::new(|| {
    let style = Css::new("style", include_str!("themes/style.css"));
    let light = Css::new("light", &LIGHT_CSS);
    let dark = Css::new("dark", &DARK_CSS);
    let index = Js::new(include_str!("javascript/index.js"));
    let paste = Js::new(include_str!("javascript/paste.js"));
    let syntax_set: SyntaxSet =
        syntect::dumps::from_binary(include_bytes!("../assets/newlines.packdump"));
    let mut syntaxes = syntax_set.syntaxes().to_vec();
    syntaxes.sort_by(|a, b| a.name.partial_cmp(&b.name).unwrap_or(Ordering::Less));

    Data {
        style,
        light,
        dark,
        index,
        paste,
        syntax_set,
        syntaxes,
    }
});

/// Combines CSS content with a filename containing the hash of the content.
pub struct Css<'a> {
    pub name: String,
    pub content: &'a str,
}

/// Javascript content.
pub struct Js<'a> {
    pub content: &'a str,
}

pub struct Data<'a> {
    pub style: Css<'a>,
    pub light: Css<'a>,
    pub dark: Css<'a>,
    pub index: Js<'a>,
    pub paste: Js<'a>,
    pub syntax_set: SyntaxSet,
    pub syntaxes: Vec<SyntaxReference>,
}

impl<'a> Css<'a> {
    fn new(name: &str, content: &'a str) -> Self {
        let name = format!(
            "{name}.{}.css",
            hex::encode(Sha256::digest(content.as_bytes()))
                .get(0..16)
                .expect("at least 16 characters")
        );

        Self { name, content }
    }
}

impl<'a> Js<'a> {
    fn new(content: &'a str) -> Self {
        Self { content }
    }
}

fn highlight(source: &str, ext: &str) -> Result<String, Error> {
    let syntax_ref = DATA
        .syntax_set
        .find_syntax_by_extension(ext)
        .unwrap_or_else(|| {
            DATA.syntax_set
                .find_syntax_by_extension("txt")
                .expect("finding txt syntax")
        });

    let mut parse_state = ParseState::new(syntax_ref);
    let mut html = String::from("<table><tbody>");
    let mut scope_stack = ScopeStack::new();

    for (mut line_number, line) in LinesWithEndings::from(source).enumerate() {
        let (formatted, delta) = if line.len() > HIGHLIGHT_LINE_LENGTH_CUTOFF {
            (line.to_string(), 0)
        } else {
            let parsed = parse_state.parse_line(line, &DATA.syntax_set)?;
            line_tokens_to_classed_spans(
                line,
                parsed.as_slice(),
                ClassStyle::Spaced,
                &mut scope_stack,
            )?
        };

        line_number += 1;
        let line_number = format!(
            r#"<tr><td class="line-number" id="L{line_number}"><a href=#L{line_number}>{line_number:>4}</a></td>"#
        );
        html.push_str(&line_number);
        html.push_str(r#"<td class="line">"#);

        if delta < 0 {
            html.push_str(&"<span>".repeat(delta.abs().try_into()?));
        }

        // Strip stray newlines that cause vertically stretched lines.
        for c in formatted.chars().filter(|c| *c != '\n') {
            html.push(c);
        }

        if delta > 0 {
            html.push_str(&"</span>".repeat(delta.try_into()?));
        }

        html.push_str("</td></tr>");
    }

    html.push_str("</tbody></table>");

    Ok(html)
}

impl Html {
    pub async fn from(entry: Entry, ext: String) -> Result<Self, Error> {
        Ok(Self(
            tokio::task::spawn_blocking(move || highlight(&entry.text, &ext)).await??,
        ))
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}
