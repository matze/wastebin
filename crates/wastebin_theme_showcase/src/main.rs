#![expect(clippy::unwrap_used)]
#![expect(clippy::print_stdout)]

use askama::Template;
use wastebin_highlight::Theme;

#[derive(Template)]
#[template(path = "page.html")]
struct Page {
    examples: Vec<Example>,
}

struct Example {
    name: &'static str,
    light_html: String,
    dark_html: String,
}

fn main() {
    let code = include_str!("main.rs");
    let syntax_set = two_face::syntax::extra_newlines();
    let syntax = syntax_set
        .syntaxes()
        .iter()
        .find(|s| s.name == "Rust")
        .unwrap();

    let highlight = |theme: &syntect::highlighting::Theme| {
        syntect::html::highlighted_html_for_string(code, &syntax_set, syntax, theme).unwrap()
    };

    let examples = [
        Theme::Ayu,
        Theme::Base16Ocean,
        Theme::Catppuccin,
        Theme::Coldark,
        Theme::Gruvbox,
        Theme::Monokai,
        Theme::Onehalf,
        Theme::Solarized,
    ]
    .into_iter()
    .map(|theme| Example {
        name: theme.name(),
        light_html: highlight(&theme.light_theme()),
        dark_html: highlight(&theme.dark_theme()),
    })
    .collect();

    println!("{}", Page { examples }.render().unwrap());
}
