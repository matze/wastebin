#![expect(clippy::unwrap_used)]
#![expect(clippy::print_stdout)]

use askama::Template;
use wastebin_highlight::{Highlighter, Theme, markdown};

/// Server stylesheet so previews match the running application.
const STYLE: &str = include_str!("../../wastebin_server/src/style.css");

/// Home icon paths.
const HOME: &str = concat!(
    "<path d=\"m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z\"/>",
    "<path d=\"M9 22V12h6v10\"/>",
);

/// Source toggle icon paths, shown on the rendered view.
const SOURCE: &str = concat!("<path d=\"m16 18 6-6-6-6\"/>", "<path d=\"m8 6-6 6 6 6\"/>",);

/// Rendered toggle icon path, shown on the source view.
const RENDERED: &str = concat!(
    "<path d=\"M22.269 19.385H1.731A1.73 1.73 0 0 1 0 17.654V6.345a1.73 1.73 0 0 1 1.731-1.73h20.538A1.73",
    " 1.73 0 0 1 24 6.345v11.308a1.73 1.73 0 0 1-1.731 1.731zm-16.5-3.462v-4.5l2.308 2.885 2.307-2.885v4.5h2.308",
    "V8.078h-2.308l-2.307 2.885-2.308-2.885H3.46v7.847zM21.231 12h-2.308V8.077h-2.307V12h-2.308l3.461 4.039z\"/>",
);

/// Moon icon paths.
const MOON: &str = "<path d=\"M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z\"/>";

/// Sun icon paths.
const SUN: &str = concat!(
    "<circle cx=\"12\" cy=\"12\" r=\"4\"/>",
    "<path d=\"M12 2v2\"/><path d=\"M12 20v2\"/>",
    "<path d=\"m4.93 4.93 1.41 1.41\"/><path d=\"m17.66 17.66 1.41 1.41\"/>",
    "<path d=\"M2 12h2\"/><path d=\"M20 12h2\"/>",
    "<path d=\"m6.34 17.66-1.41 1.41\"/><path d=\"m19.07 4.93-1.41 1.41\"/>",
);

/// Monitor icon paths.
const MONITOR: &str = concat!(
    "<rect width=\"20\" height=\"14\" x=\"2\" y=\"3\" rx=\"2\"/>",
    "<path d=\"M8 21h8\"/><path d=\"M12 17v4\"/>",
);

/// Markdown source rendered in the rendered view.
const SAMPLE: &str = r#"# Rendered Markdown

The chrome around this text comes from the same stylesheet the server serves, and the
colors come from the palette derived for the theme. Body text, borders, accents and
admonitions all follow `WASTEBIN_THEME`. A [link](https://example.com) uses the accent,
`inline code` uses the inset surface, **bold** and *italic* use the foreground.

## Quote and admonitions

> A plain blockquote, styled like the code blocks.

> [!NOTE]
> Notes use the theme's tag color.

> [!TIP]
> Tips use the string color.

> [!IMPORTANT]
> Important uses the keyword color.

> [!WARNING]
> Warning uses the type color, or a matching hue when the theme has none.

> [!CAUTION]
> Caution uses the danger color.

## Code, tables and tasks

```rust
fn main() {
    let payload = "hello";
    println!("{payload}");
}
```

| Surface | Token | Source |
| --- | --- | --- |
| Page | `--page-bg` | theme background |
| Panel | `--panel-bg` | background raised toward white |
| Inset | `--panel-bg2` | background mixed with the foreground |
| Code | `--code-fg` | theme foreground, unadjusted |

- [x] rendered
- [ ] pending

---

Last line.
"#;

/// Rust source shown in the source view.
const SAMPLE_SOURCE: &str = include_str!("main.rs");

#[derive(Template)]
#[template(path = "page.html")]
struct Page {
    groups: Vec<Group>,
}

struct Group {
    name: &'static str,
    panes: Vec<Pane>,
}

struct Pane {
    scheme: &'static str,
    view: View,
    document: String,
}

/// One of the two example views.
#[derive(Copy, Clone, PartialEq, Eq)]
enum View {
    /// Rendered Markdown.
    Rendered,
    /// Line-numbered source.
    Source,
}

impl View {
    /// Name shown in the gallery caption.
    fn name(self) -> &'static str {
        match self {
            Self::Rendered => "rendered",
            Self::Source => "source",
        }
    }
}

/// Rendered sources.
struct Content {
    markdown: String,
    source: String,
}

fn main() {
    let highlighter = Highlighter::default();
    let content = Content {
        markdown: markdown::render(SAMPLE, &highlighter).unwrap().into_inner(),
        source: highlighter
            .highlight(String::from(SAMPLE_SOURCE), Some(String::from("rs")))
            .unwrap()
            .into_inner(),
    };

    let groups = [
        Theme::Ayu,
        Theme::Base16Ocean,
        Theme::Catppuccin,
        Theme::Coldark,
        Theme::Gruvbox,
        Theme::Monokai,
        Theme::Onehalf,
        Theme::RosePine,
        Theme::Solarized,
    ]
    .into_iter()
    .map(|theme| Group {
        name: theme.name(),
        panes: ["light", "dark"]
            .into_iter()
            .flat_map(|scheme| {
                [View::Rendered, View::Source]
                    .into_iter()
                    .map(move |view| (scheme, view))
            })
            .map(|(scheme, view)| Pane {
                scheme,
                view,
                document: document(scheme, view, &theme, &content),
            })
            .collect(),
    })
    .collect();

    println!("{}", Page { groups }.render().unwrap());
}

/// Complete document for one preview pane, styled like the application.
fn document(scheme: &str, view: View, theme: &Theme, content: &Content) -> String {
    let css = match scheme {
        "light" => theme.light_css(),
        _ => theme.dark_css(),
    };

    format!(
        concat!(
            "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">",
            "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">",
            "<style>{css}</style><style>{style}</style></head><body>{body}</body></html>"
        ),
        css = String::from_utf8_lossy(&css),
        style = STYLE,
        body = body(scheme, view, content),
    )
}

/// Application chrome with a single view of a paste.
fn body(scheme: &str, view: View, content: &Content) -> String {
    let (title, toggle, pane) = match view {
        View::Rendered => (
            "sample.md",
            stroke_icon(SOURCE),
            format!(
                "<article class=\"markdown-body\">{}</article>",
                content.markdown
            ),
        ),
        View::Source => (
            "sample.rs",
            fill_icon(RENDERED),
            format!("<div class=\"source-view\">{}</div>", content.source),
        ),
    };

    format!(
        concat!(
            "<div id=\"main-container\"><header><div class=\"nav-group\">",
            "<a href=\"#\" class=\"nav-button\" title=\"Home\" aria-label=\"Home\">{home}</a>",
            "<span class=\"nav-title\">{title}</span></div>",
            "<div class=\"nav-group\"><a href=\"#\" class=\"nav-button\" title=\"Toggle\" ",
            "aria-label=\"Toggle\">{toggle}</a>",
            "<div class=\"theme-switcher\">{switcher}</div></div></header>",
            "<div id=\"content-area\"><main>{pane}</main></div></div>"
        ),
        home = stroke_icon(HOME),
        title = title,
        toggle = toggle,
        switcher = theme_switcher(scheme),
        pane = pane,
    )
}

/// Stroked svg element for a nav button.
fn stroke_icon(paths: &str) -> String {
    format!(
        concat!(
            "<svg viewBox=\"0 0 24 24\" width=\"16\" height=\"16\" fill=\"none\" stroke=\"currentColor\" ",
            "stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{paths}</svg>"
        ),
        paths = paths,
    )
}

/// Filled svg element for the rendered view toggle.
fn fill_icon(paths: &str) -> String {
    format!(
        "<svg viewBox=\"0 0 24 24\" width=\"16\" height=\"16\" fill=\"currentColor\" stroke=\"none\">{paths}</svg>",
        paths = paths,
    )
}

/// Theme switcher with `scheme` selected.
fn theme_switcher(scheme: &str) -> String {
    [
        ("dark", MOON, "dark mode"),
        ("light", SUN, "light mode"),
        ("system", MONITOR, "auto mode"),
    ]
    .into_iter()
    .map(|(name, paths, title)| {
        let class = if name == scheme {
            "theme-opt active"
        } else {
            "theme-opt"
        };
        format!(
            "<a href=\"#\" class=\"{class}\" title=\"{title}\" aria-label=\"{title}\">{icon}</a>",
            icon = stroke_icon(paths),
        )
    })
    .collect()
}
