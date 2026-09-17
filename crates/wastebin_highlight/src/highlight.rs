use std::fmt::Write;

use syntect::html::{ClassStyle, ClassedHTMLGenerator, line_tokens_to_classed_spans};
use syntect::parsing::{
    BasicScopeStackOp, ParseState, Scope, ScopeStack, ScopeStackOp, SyntaxReference, SyntaxSet,
};
use syntect::util::LinesWithEndings;

#[expect(deprecated)]
use syntect::parsing::SCOPE_REPO;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("syntax highlighting error: {0}")]
    SyntaxHighlighting(#[from] syntect::Error),
    #[error("syntax parsing error: {0}")]
    SyntaxParsing(#[from] syntect::parsing::ParsingError),
}

const HIGHLIGHT_LINE_LENGTH_CUTOFF: usize = 2048;

#[derive(Clone)]
pub struct Html(String);

#[derive(Clone)]
pub struct Highlighter {
    syntax_set: SyntaxSet,
    ordered_syntaxes: Vec<SyntaxReference>,
}

/// Syntax reference.
pub struct Syntax<'a> {
    /// Name of the syntax or the language it is related to.
    pub name: &'a str,
    /// List of possible filename extensions.
    pub extensions: &'a [String],
}

impl Default for Highlighter {
    fn default() -> Self {
        let syntax_set = two_face::syntax::extra_newlines();
        let mut syntaxes = syntax_set.syntaxes().to_vec();
        syntaxes.sort_unstable_by_key(|s| s.name.to_lowercase());

        Self {
            syntax_set,
            ordered_syntaxes: syntaxes,
        }
    }
}

/// Escape HTML tags in `s` and write output to `buf`.
fn escape(s: &str, buf: &mut String) -> std::fmt::Result {
    // Because the internet is always right, turns out there's not that many
    // characters to escape: http://stackoverflow.com/questions/7381974
    let pile_o_bits = s;
    let mut last = 0;
    for (i, ch) in s.bytes().enumerate() {
        let escaping = match ch as char {
            '>' => "&gt;",
            '<' => "&lt;",
            '&' => "&amp;",
            '\'' => "&#39;",
            '"' => "&quot;",
            _ => continue,
        };

        buf.write_str(&pile_o_bits[last..i])?;
        buf.write_str(escaping)?;
        last = i + 1;
    }

    if last < s.len() {
        buf.write_str(&pile_o_bits[last..])?;
    }

    Ok(())
}

/// Transform `scope` atoms to CSS style classes and write output to `s`.
fn scope_to_classes(s: &mut String, scope: Scope) {
    #[expect(deprecated)]
    let repo = SCOPE_REPO.lock().expect("lock");
    for i in 0..(scope.len()) {
        let atom = scope.atom_at(i as usize);
        let atom_s = repo.atom_str(atom);
        if i != 0 {
            s.push(' ');
        }
        s.push_str(atom_s);
    }
}

/// Return `true` if `scope` will be used to render a Markdown link.
fn is_markdown_link(scope: Scope) -> bool {
    #[expect(deprecated)]
    let repo = SCOPE_REPO.lock().expect("lock");

    (0..scope.len()).all(|index| {
        let atom = repo.atom_str(scope.atom_at(index as usize));
        atom == "markup" || atom == "underline" || atom == "link" || atom == "markdown"
    })
}

/// Number of unmatched `</span>` closes encountered before the running balance recovers.
fn open_span_prefix(formatted: &str) -> usize {
    if !formatted.contains("<span") && !formatted.contains("</span>") {
        return 0;
    }

    formatted
        .split('<')
        .skip(1)
        .scan(0_isize, |balance, chunk| {
            if chunk.starts_with("/span>") {
                *balance -= 1;
            } else if chunk.starts_with("span>") || chunk.starts_with("span ") {
                *balance += 1;
            }
            Some(*balance)
        })
        .min()
        .map(std::ops::Neg::neg)
        .unwrap_or(0)
        .try_into()
        .unwrap_or(0)
}

/// Modified version of [`syntect::html::line_tokens_to_classed_spans`] that outputs HTML anchors
/// for Markdown links.
fn line_tokens_to_classed_spans_md(
    line: &str,
    ops: &[(usize, ScopeStackOp)],
    stack: &mut ScopeStack,
) -> Result<(String, isize), syntect::Error> {
    let mut s = String::with_capacity(line.len() + ops.len() * 8); // a guess
    let mut cur_index = 0;
    let mut span_delta = 0;

    let mut span_empty = false;
    let mut span_start = 0;
    let mut handling_link = false;

    for &(i, ref op) in ops {
        if i > cur_index {
            span_empty = false;

            if handling_link {
                // Insert href and close attribute ...
                escape(&line[cur_index..i], &mut s)?;
                s.push_str(r#"">"#);
            }

            escape(&line[cur_index..i], &mut s)?;

            cur_index = i;
        }
        stack.apply_with_hook(op, |basic_op, _| match basic_op {
            BasicScopeStackOp::Push(scope) => {
                span_start = s.len();
                span_empty = true;
                s.push_str("<span class=\"");
                scope_to_classes(&mut s, scope);
                s.push_str("\">");
                span_delta += 1;

                if is_markdown_link(scope) {
                    s.push_str(r#"<a href=""#);
                    handling_link = true;
                }
            }
            BasicScopeStackOp::Pop => {
                if handling_link {
                    s.push_str("</a>");
                    handling_link = false;
                }
                if span_empty {
                    s.truncate(span_start);
                } else {
                    s.push_str("</span>");
                }
                span_delta -= 1;
                span_empty = false;
            }
        })?;
    }
    escape(&line[cur_index..line.len()], &mut s)?;
    Ok((s, span_delta))
}

impl Highlighter {
    /// Highlight `text` with the given file extension which is used to
    /// determine the right syntax. If not given or does not exist, plain text will be generated.
    pub fn highlight(&self, text: String, ext: Option<String>) -> Result<Html, Error> {
        let syntax_ref = self
            .syntax_set
            .find_syntax_by_extension(ext.as_deref().unwrap_or("txt"))
            .unwrap_or_else(|| {
                self.syntax_set
                    .find_syntax_by_extension("txt")
                    .expect("finding txt syntax")
            });

        let is_markdown = syntax_ref.name == "Markdown";
        let mut parse_state = ParseState::new(syntax_ref);
        let mut html = String::from(r#"<div id="line-numbers" aria-hidden="true">"#);
        let mut code = String::from(r#"<div class="src-code"><code>"#);
        let mut scope_stack = ScopeStack::new();

        for (mut line_number, line) in LinesWithEndings::from(&text).enumerate() {
            let (formatted, delta) = if line.len() > HIGHLIGHT_LINE_LENGTH_CUTOFF {
                // Skipping the highlighter must not skip escaping: `formatted` is written
                // into the page as raw HTML further down.
                let mut plain = String::with_capacity(line.len());
                escape(line, &mut plain).map_err(syntect::Error::from)?;
                (plain, 0)
            } else {
                let parsed = parse_state.parse_line(line, &self.syntax_set)?;

                if is_markdown {
                    line_tokens_to_classed_spans_md(line, parsed.as_slice(), &mut scope_stack)?
                } else {
                    line_tokens_to_classed_spans(
                        line,
                        parsed.as_slice(),
                        ClassStyle::Spaced,
                        &mut scope_stack,
                    )?
                }
            };

            line_number += 1;
            let _ = write!(
                html,
                r##"<div id="L{line_number}"><a href="#L{line_number}">{line_number}</a></div>"##
            );

            let _ = write!(code, r#"<div id="LC{line_number}">"#);

            // The line may close spans opened on earlier lines before opening any of its own.
            // Track the minimum running span balance so we can prepend bare `<span>`s to keep
            // the line's HTML self-contained — using only `delta` would let `</span>` precede
            // its match within the line, producing misnested output.
            let prepend = open_span_prefix(&formatted);
            code.push_str(&"<span>".repeat(prepend));

            code.reserve(formatted.len());

            for segment in formatted.split('\n') {
                code.push_str(segment);
            }

            let extra_close =
                isize::try_from(prepend).expect("prepend count fits into isize") + delta;

            if extra_close > 0 {
                code.push_str(
                    &"</span>".repeat(extra_close.try_into().expect("isize fits into usize")),
                );
            }

            code.push_str("</div>");
        }

        html.push_str("</div>");
        code.push_str("</code></div>");
        html.push_str(&code);

        Ok(Html(html))
    }

    /// Highlight a fenced code block. `token` is the info string (e.g. `rust`, `py`); unknown or
    /// empty tokens fall back to plain text. Unlike [`Highlighter::highlight`], the output is a
    /// compact `<pre><code>` without line numbers, suitable for embedding into rendered Markdown.
    pub fn highlight_code_block(&self, text: &str, token: &str) -> Result<String, Error> {
        let syntax = self
            .syntax_set
            .find_syntax_by_token(token)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut generator = ClassedHTMLGenerator::new_with_class_style(
            syntax,
            &self.syntax_set,
            ClassStyle::Spaced,
        );

        for line in LinesWithEndings::from(text) {
            generator.parse_html_for_line_which_includes_newline(line)?;
        }

        let inner = generator.finalize();
        let is_safe_token = !token.is_empty()
            && token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        let class = if is_safe_token {
            format!("code-block language-{token}")
        } else {
            String::from("code-block")
        };

        Ok(format!("<pre class=\"{class}\"><code>{inner}</code></pre>"))
    }

    /// Return iterator over all available [`Syntax`]es with their canonical name and usual file
    /// extensions.
    pub fn syntaxes(&self) -> impl Iterator<Item = Syntax<'_>> {
        self.ordered_syntaxes.iter().map(|syntax| Syntax {
            name: syntax.name.as_ref(),
            extensions: syntax.file_extensions.as_slice(),
        })
    }
}

impl Html {
    /// Wrap an already-HTML string. Callers are responsible for ensuring the content is safe to
    /// insert into a page (i.e. produced by a trusted renderer).
    pub fn new(html: String) -> Self {
        Self(html)
    }

    pub fn into_inner(self) -> String {
        self.0
    }

    /// Length in bytes of the wrapped HTML.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return `true` if the wrapped HTML is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_lines_are_escaped() -> Result<(), Box<dyn std::error::Error>> {
        let highlighter = Highlighter::default();
        let payload = "<img src=x onerror=alert(1)>";
        let line = format!("{}{payload}", "A".repeat(HIGHLIGHT_LINE_LENGTH_CUTOFF));
        assert!(line.len() > HIGHLIGHT_LINE_LENGTH_CUTOFF);

        let html = highlighter.highlight(line, None)?.into_inner();

        assert!(
            !html.contains(payload),
            "long line reached the page unescaped"
        );
        assert!(html.contains("&lt;img src=x onerror=alert(1)&gt;"));

        Ok(())
    }

    #[test]
    fn markdown_links() -> Result<(), Box<dyn std::error::Error>> {
        let highlighter = Highlighter::default();

        let html = highlighter.highlight(
            "[hello](https://github.com/matze/wastebin)".into(),
            Some("md".into()),
        )?;

        assert!(html.into_inner().contains("<span class=\"markup underline link markdown\"><a href=\"https://github.com/matze/wastebin\">https://github.com/matze/wastebin</a></span>"));

        Ok(())
    }

    /// Per-row HTML must be self-balanced: every `</span>` should have a matching `<span>`
    /// earlier on the same row. Returns the minimum running balance encountered.
    fn min_span_balance(row: &str) -> isize {
        let bytes = row.as_bytes();
        let mut i = 0;
        let mut balance: isize = 0;
        let mut min_balance: isize = 0;
        while i < bytes.len() {
            let rest = &row[i..];
            if rest.starts_with("</span>") {
                balance -= 1;
                if balance < min_balance {
                    min_balance = balance;
                }
                i += "</span>".len();
            } else if rest.starts_with("<span") && matches!(bytes.get(i + 5), Some(b' ' | b'>')) {
                balance += 1;
                i += rest.find('>').map_or(1, |c| c + 1);
            } else {
                i += 1;
            }
        }
        assert_eq!(balance, 0, "row not balanced: {row}");
        min_balance
    }

    #[test]
    fn rows_are_self_balanced_for_markdown_lists() -> Result<(), Box<dyn std::error::Error>> {
        let highlighter = Highlighter::default();
        let text = "## Features\n\n\
            * [axum](https://github.com/tokio-rs/axum) and [sqlite3](https://www.sqlite.org) backend\n\
            * comes as a single binary with low memory footprint\n";
        let html = highlighter
            .highlight(text.into(), Some("md".into()))?
            .into_inner();

        for row in html.split("</div>").filter(|s| s.contains("id=\"LC")) {
            assert!(
                min_span_balance(row) >= 0,
                "row has unmatched </span>: {row}"
            );
        }
        Ok(())
    }

    #[test]
    fn markdown_link_is_well_nested() -> Result<(), Box<dyn std::error::Error>> {
        let highlighter = Highlighter::default();
        let html = highlighter
            .highlight("[hi](https://example.com)".into(), Some("md".into()))?
            .into_inner();

        assert!(
            !html.contains("</span></a>"),
            "anchor must close before its enclosing span: {html}"
        );
        Ok(())
    }
}
