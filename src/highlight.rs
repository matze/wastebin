use crate::Error;
use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};
use tree_painter::{Lang, Renderer, Theme};

pub static DATA: Lazy<Data> = Lazy::new(|| {
    let light_theme = Theme::from_helix(&tree_painter::themes::CATPPUCCIN_LATTE).unwrap();
    let dark_theme = Theme::from_helix(&tree_painter::themes::CATPPUCCIN_MOCHA).unwrap();
    let renderer = Renderer::new(light_theme);
    let light = renderer.css();
    let dark = Renderer::new(dark_theme).css();

    Data {
        main: include_str!("themes/style.css"),
        renderer: Arc::new(Mutex::new(renderer)),
        light,
        dark,
    }
});

pub struct Data<'a> {
    pub main: &'a str,
    pub dark: String,
    pub light: String,
    pub renderer: Arc<Mutex<Renderer>>,
}

fn highlight_real(text: &str, lang: Lang) -> Result<String, Error> {
    let mut html = String::from("<table><tbody>");
    let mut renderer = DATA.renderer.lock().unwrap();

    for (mut line_number, line) in renderer.render(&lang, text.as_bytes())?.enumerate() {
        line_number += 1;

        html.push_str(&format!(
            r#"<tr><td class="line-number"><a href=#L{line_number}>{line_number:>4}</a></td>"#
        ));
        html.push_str(&format!(r#"<td class="tsc-line">{line}</td></tr>"#));
    }

    html.push_str("</tbody></table>");
    Ok(html)
}

fn highlight_plain(text: &str) -> String {
    let mut html = String::from("<table><tbody>");

    for (mut line_number, line) in text.lines().enumerate() {
        line_number += 1;
        html.push_str(&format!(
            r#"<tr><td class="line-number"><a href=#L{line_number}>{line_number:>4}</a></td>"#
        ));
        html.push_str(&format!(r#"<td class="tsc-line">{line}</td></tr>"#));
    }

    html.push_str("</tbody></table>");
    html
}

pub fn highlight(text: &str, ext: &str) -> Result<String, Error> {
    match Lang::from_extension(ext) {
        Some(lang) => highlight_real(text, lang),
        None => Ok(highlight_plain(text)),
    }
}
