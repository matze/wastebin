mod nesting;

use std::sync::OnceLock;

use ammonia::Builder;
use pulldown_cmark::{BlockQuoteKind, CodeBlockKind, CowStr, Event, Options, Parser, Tag, html};

use crate::highlight::Error;
use crate::{Highlighter, Html};
use nesting::nesting_depth;

/// Deepest markup nesting handed to the sanitizer.
///
/// Building the DOM is quadratic in nesting depth, and raw HTML in a paste reaches it verbatim:
/// a megabyte of `<div>` is a quarter million levels deep and costs minutes of CPU. Prose does not
/// come close to this bound — deeply nested lists sit around a dozen levels.
const MAX_NESTING_DEPTH: usize = 256;

/// Render CommonMark `text` to HTML. Fenced code blocks with a known language are syntax
/// highlighted via `highlighter`; unknown languages fall back to plain text.
///
/// Raw HTML embedded in the source is passed through the parser and then sanitized by
/// [`ammonia`], so tags like `<details>` or `<kbd>` survive while `<script>`, inline event
/// handlers, `javascript:` URLs and other XSS vectors are stripped.
///
/// Markup nested deeper than [`MAX_NESTING_DEPTH`] is rejected instead of sanitized.
pub fn render(text: &str, highlighter: &Highlighter) -> Result<Html, Error> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_GFM;

    let parser = Parser::new_ext(text, options);
    let events = rewrite_events(parser, highlighter)?;

    let mut raw = String::with_capacity(text.len());
    html::push_html(&mut raw, events.into_iter());

    let depth = nesting_depth(&raw);
    if depth > MAX_NESTING_DEPTH {
        return Err(Error::TooDeeplyNested(MAX_NESTING_DEPTH));
    }

    Ok(Html::new(sanitizer().clean(&raw).to_string()))
}

fn rewrite_events<'a>(
    parser: Parser<'a>,
    highlighter: &Highlighter,
) -> Result<Vec<Event<'a>>, Error> {
    let mut out = Vec::new();
    let mut pending: Option<(String, String)> = None;

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                pending = Some((lang.to_string(), String::new()));
            }
            Event::Text(text) if pending.is_some() => {
                if let Some((_, buf)) = pending.as_mut() {
                    buf.push_str(&text);
                }
            }
            Event::End(pulldown_cmark::TagEnd::CodeBlock) if pending.is_some() => {
                if let Some((lang, code)) = pending.take() {
                    let html = highlighter.highlight_code_block(&code, &lang)?;
                    out.push(Event::Html(CowStr::from(html)));
                }
            }
            Event::Start(Tag::BlockQuote(Some(kind))) => {
                out.push(Event::Start(Tag::BlockQuote(Some(kind))));
                out.push(Event::Html(CowStr::from(alert_title(kind))));
            }
            other => out.push(other),
        }
    }

    Ok(out)
}

/// Shared ammonia sanitizer. Extends the default allowlist with `class` on any tag (needed for
/// syntax-highlight spans and alert blockquotes) and the handful of attributes pulldown-cmark
/// emits on task-list checkboxes.
fn sanitizer() -> &'static Builder<'static> {
    static CLEANER: OnceLock<Builder<'static>> = OnceLock::new();
    CLEANER.get_or_init(|| {
        let mut builder = Builder::default();
        builder.add_generic_attributes(["class"]);
        builder.add_tags(["input"]);
        builder.add_tag_attributes("input", ["type", "checked", "disabled"]);
        builder
    })
}

/// Return the HTML injected at the top of a GFM alert blockquote.
fn alert_title(kind: BlockQuoteKind) -> String {
    let label = match kind {
        BlockQuoteKind::Note => "Note",
        BlockQuoteKind::Tip => "Tip",
        BlockQuoteKind::Important => "Important",
        BlockQuoteKind::Warning => "Warning",
        BlockQuoteKind::Caution => "Caution",
    };
    format!("<p class=\"markdown-alert-title\">{label}</p>")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_string(text: &str, highlighter: &Highlighter) -> Result<String, Error> {
        render(text, highlighter).map(Html::into_inner)
    }

    #[test]
    fn deeply_nested_markup_is_rejected() {
        let md = "<div>".repeat(MAX_NESTING_DEPTH + 10);
        let result = render(&md, &Highlighter::default());
        assert!(matches!(result, Err(Error::TooDeeplyNested(_))));
    }

    #[test]
    fn nesting_hidden_behind_unmatched_close_tags_is_rejected() {
        let md = "<div></span>".repeat(MAX_NESTING_DEPTH + 10);
        let result = render(&md, &Highlighter::default());
        assert!(matches!(result, Err(Error::TooDeeplyNested(_))));
    }

    /// HTML5 ignores a trailing slash on anything but a void element, so `<div/>` opens a level
    /// just like `<div>`.
    #[test]
    fn a_slash_does_not_close_a_non_void_element() {
        let md = "<div/>".repeat(MAX_NESTING_DEPTH + 10);
        let result = render(&md, &Highlighter::default());
        assert!(matches!(result, Err(Error::TooDeeplyNested(_))));
    }

    #[test]
    fn nesting_within_the_limit_still_renders() -> Result<(), Box<dyn std::error::Error>> {
        let md = "<div>".repeat(32);
        let html = render_string(&md, &Highlighter::default())?;
        assert!(html.contains("<div>"), "got: {html}");
        Ok(())
    }

    #[test]
    fn flat_markup_is_not_mistaken_for_nesting() -> Result<(), Box<dyn std::error::Error>> {
        // Siblings and void elements open no levels, so a long flat document must render.
        let md = format!("{}\n\n{}", "<div>x</div>".repeat(500), "<br>".repeat(500));
        let html = render_string(&md, &Highlighter::default())?;
        assert!(html.contains("<div>"), "got: {html}");
        Ok(())
    }

    #[test]
    fn heading() -> Result<(), Box<dyn std::error::Error>> {
        let html = render_string("# Hello", &Highlighter::default())?;
        assert!(html.contains("<h1>Hello</h1>"), "got: {html}");
        Ok(())
    }

    #[test]
    fn table() -> Result<(), Box<dyn std::error::Error>> {
        let md = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let html = render_string(md, &Highlighter::default())?;
        assert!(html.contains("<table>"), "got: {html}");
        assert!(html.contains("<th>a</th>"), "got: {html}");
        Ok(())
    }

    #[test]
    fn task_list() -> Result<(), Box<dyn std::error::Error>> {
        let html = render_string("- [x] done\n- [ ] open\n", &Highlighter::default())?;
        assert!(html.contains("type=\"checkbox\""), "got: {html}");
        assert!(html.contains("checked"), "got: {html}");
        Ok(())
    }

    #[test]
    fn strikethrough() -> Result<(), Box<dyn std::error::Error>> {
        let html = render_string("~~gone~~", &Highlighter::default())?;
        assert!(html.contains("<del>gone</del>"), "got: {html}");
        Ok(())
    }

    #[test]
    fn code_block_is_highlighted() -> Result<(), Box<dyn std::error::Error>> {
        let md = "```rust\nfn main() {}\n```\n";
        let html = render_string(md, &Highlighter::default())?;
        assert!(
            html.contains("class=\"code-block language-rust\""),
            "got: {html}"
        );
        assert!(html.contains("<span class=\""), "got: {html}");
        Ok(())
    }

    #[test]
    fn code_block_unknown_language_falls_back() -> Result<(), Box<dyn std::error::Error>> {
        let md = "```not-a-real-lang\nhello\n```\n";
        let html = render_string(md, &Highlighter::default())?;
        assert!(html.contains("<pre"), "got: {html}");
        assert!(html.contains("hello"), "got: {html}");
        Ok(())
    }

    #[test]
    fn code_block_without_language() -> Result<(), Box<dyn std::error::Error>> {
        let md = "```\nraw\n```\n";
        let html = render_string(md, &Highlighter::default())?;
        assert!(html.contains("class=\"code-block\""), "got: {html}");
        assert!(!html.contains("language-"), "got: {html}");
        Ok(())
    }

    #[test]
    fn code_block_malicious_language_is_sanitized() -> Result<(), Box<dyn std::error::Error>> {
        let md = "```\"><script>\ncode\n```\n";
        let html = render_string(md, &Highlighter::default())?;
        assert!(!html.contains("<script>"), "got: {html}");
        Ok(())
    }

    #[test]
    fn gfm_alert_note() -> Result<(), Box<dyn std::error::Error>> {
        let md = "> [!NOTE]\n> body\n";
        let html = render_string(md, &Highlighter::default())?;
        assert!(
            html.contains("<blockquote class=\"markdown-alert-note\">"),
            "got: {html}"
        );
        assert!(
            html.contains("<p class=\"markdown-alert-title\">Note</p>"),
            "got: {html}"
        );
        assert!(html.contains("body"), "got: {html}");
        Ok(())
    }

    #[test]
    fn gfm_alert_variants() -> Result<(), Box<dyn std::error::Error>> {
        for (marker, class, label) in [
            ("TIP", "markdown-alert-tip", "Tip"),
            ("IMPORTANT", "markdown-alert-important", "Important"),
            ("WARNING", "markdown-alert-warning", "Warning"),
            ("CAUTION", "markdown-alert-caution", "Caution"),
        ] {
            let md = format!("> [!{marker}]\n> body\n");
            let html = render_string(&md, &Highlighter::default())?;
            assert!(html.contains(class), "{marker}: {html}");
            assert!(
                html.contains(&format!("<p class=\"markdown-alert-title\">{label}</p>")),
                "{marker}: {html}"
            );
        }
        Ok(())
    }

    #[test]
    fn plain_blockquote_is_not_an_alert() -> Result<(), Box<dyn std::error::Error>> {
        let html = render_string("> just a quote\n", &Highlighter::default())?;
        assert!(html.contains("<blockquote>"), "got: {html}");
        assert!(!html.contains("markdown-alert"), "got: {html}");
        Ok(())
    }

    #[test]
    fn dangerous_raw_html_is_stripped() -> Result<(), Box<dyn std::error::Error>> {
        let md =
            "<script>alert(1)</script>\n\n<a href=\"javascript:alert(1)\" onclick=\"x\">x</a>\n";
        let html = render_string(md, &Highlighter::default())?;
        assert!(!html.contains("<script"), "got: {html}");
        assert!(!html.contains("alert(1)"), "got: {html}");
        assert!(!html.contains("javascript:"), "got: {html}");
        assert!(!html.contains("onclick"), "got: {html}");
        Ok(())
    }

    #[test]
    fn safe_raw_html_survives() -> Result<(), Box<dyn std::error::Error>> {
        let md = "<details><summary>more</summary>hidden</details>\n\nPress <kbd>Ctrl</kbd>.\n";
        let html = render_string(md, &Highlighter::default())?;
        assert!(html.contains("<details>"), "got: {html}");
        assert!(html.contains("<summary>more</summary>"), "got: {html}");
        assert!(html.contains("<kbd>Ctrl</kbd>"), "got: {html}");
        Ok(())
    }
}
