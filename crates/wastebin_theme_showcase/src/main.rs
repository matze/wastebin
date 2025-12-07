#![expect(clippy::unwrap_used)]
#![expect(clippy::print_stdout)]

use askama::Template;

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
        (wastebin_highlight::Theme::Ayu, "ayu"),
        (wastebin_highlight::Theme::Base16Ocean, "base16ocean"),
        (wastebin_highlight::Theme::Catppuccin, "catppuccin"),
        (wastebin_highlight::Theme::Coldark, "coldark"),
        (wastebin_highlight::Theme::Gruvbox, "gruvbox"),
        (wastebin_highlight::Theme::Monokai, "monokai"),
        (wastebin_highlight::Theme::Onehalf, "onehalf"),
        (wastebin_highlight::Theme::Solarized, "solarized"),
    ]
    .into_iter()
    .map(|(theme, name)| Example {
        name,
        light_html: highlight(&theme.light_theme()),
        dark_html: highlight(&theme.dark_theme()),
    })
    .collect();

    println!("{}", Page { examples }.render().unwrap());
}
