use crate::db::read::Entry;
use crate::env;
use crate::errors::Error;
use askama::Template;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::io::Cursor;
use std::sync::LazyLock;
use syntect::highlighting::{Color, ThemeSet};
use syntect::html::{css_for_theme_with_class_style, line_tokens_to_classed_spans, ClassStyle};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;
use two_face::theme::EmbeddedThemeName;

const HIGHLIGHT_LINE_LENGTH_CUTOFF: usize = 2048;

/// Supported themes.
pub enum Theme {
    Ayu,
    Base16Ocean,
    Coldark,
    Gruvbox,
    Monokai,
    Onehalf,
    Solarized,
}

#[derive(Template)]
#[template(path = "style.css", escape = "none")]
struct StyleCss {
    light_background: Color,
    light_foreground: Color,
    dark_background: Color,
    dark_foreground: Color,
}

#[derive(Clone)]
pub struct Html(String);

pub static LIGHT_THEME: LazyLock<syntect::highlighting::Theme> = LazyLock::new(|| {
    let theme_set = two_face::theme::extra();

    match *env::THEME {
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
});

pub static LIGHT_CSS: LazyLock<String> = LazyLock::new(|| {
    css_for_theme_with_class_style(&LIGHT_THEME, ClassStyle::Spaced).expect("generating CSS")
});

pub static DARK_THEME: LazyLock<syntect::highlighting::Theme> = LazyLock::new(|| {
    let theme_set = two_face::theme::extra();

    match *env::THEME {
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
});

pub static DARK_CSS: LazyLock<String> = LazyLock::new(|| {
    css_for_theme_with_class_style(&DARK_THEME, ClassStyle::Spaced).expect("generating CSS")
});

trait ColorExt {
    fn new(r: u8, g: u8, b: u8, a: u8) -> Self;
}

impl ColorExt for Color {
    fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

pub static STYLE_CSS: LazyLock<String> = LazyLock::new(|| {
    let light_foreground = LIGHT_THEME
        .settings
        .foreground
        .unwrap_or(Color::new(3, 3, 3, 100));

    let light_background = LIGHT_THEME
        .settings
        .background
        .unwrap_or(Color::new(250, 250, 250, 100));

    let dark_foreground = DARK_THEME
        .settings
        .foreground
        .unwrap_or(Color::new(230, 225, 207, 100));

    let dark_background = DARK_THEME
        .settings
        .background
        .unwrap_or(Color::new(15, 20, 25, 100));

    let style = StyleCss {
        light_background,
        light_foreground,
        dark_background,
        dark_foreground,
    };
    style.render().expect("rendering style css")
});

pub static DATA: LazyLock<Data> = LazyLock::new(|| {
    let style = Hashed::new("style", "css", &STYLE_CSS);
    let index = Hashed::new("index", "js", include_str!("javascript/index.js"));
    let paste = Hashed::new("paste", "js", include_str!("javascript/paste.js"));
    let syntax_set = two_face::syntax::extra_newlines();
    let mut syntaxes = syntax_set.syntaxes().to_vec();
    syntaxes.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .partial_cmp(&b.name.to_lowercase())
            .unwrap_or(Ordering::Less)
    });

    Data {
        style,
        index,
        paste,
        syntax_set,
        syntaxes,
    }
});

/// Combines content with a filename containing the hash of the content.
pub struct Hashed<'a> {
    pub name: String,
    pub content: &'a str,
}

pub struct Data<'a> {
    pub style: Hashed<'a>,
    pub index: Hashed<'a>,
    pub paste: Hashed<'a>,
    pub syntax_set: SyntaxSet,
    pub syntaxes: Vec<SyntaxReference>,
}

impl<'a> Hashed<'a> {
    fn new(name: &str, ext: &str, content: &'a str) -> Self {
        let name = format!(
            "{name}.{}.{ext}",
            hex::encode(Sha256::digest(content.as_bytes()))
                .get(0..16)
                .expect("at least 16 characters")
        );

        Self { name, content }
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
