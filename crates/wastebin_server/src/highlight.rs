use crate::errors::Error;
use std::cmp::Ordering;
use std::fmt::Write;
use syntect::html::{ClassStyle, line_tokens_to_classed_spans};
use syntect::parsing::{
    BasicScopeStackOp, ParseState, Scope, ScopeStack, ScopeStackOp, SyntaxReference, SyntaxSet,
};
use syntect::util::LinesWithEndings;

#[expect(deprecated)]
use syntect::parsing::SCOPE_REPO;

const HIGHLIGHT_LINE_LENGTH_CUTOFF: usize = 2048;

/// Supported themes.
#[derive(Copy, Clone)]
pub(crate) enum Theme {
    Ayu,
    Base16Ocean,
    Catppuccin,
    Coldark,
    Gruvbox,
    Monokai,
    Onehalf,
    Solarized,
}

#[derive(Clone)]
pub(crate) struct Html(String);

#[derive(Clone)]
pub(crate) struct Highlighter {
    syntax_set: SyntaxSet,
    pub syntaxes: Vec<SyntaxReference>,
}

impl Default for Highlighter {
    fn default() -> Self {
        let syntax_set = two_face::syntax::extra_newlines();
        let mut syntaxes = syntax_set.syntaxes().to_vec();
        syntaxes.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .partial_cmp(&b.name.to_lowercase())
                .unwrap_or(Ordering::Less)
        });

        Self {
            syntax_set,
            syntaxes,
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
        match ch as char {
            '<' | '>' | '&' | '\'' | '"' => {
                buf.write_str(&pile_o_bits[last..i])?;
                let s = match ch as char {
                    '>' => "&gt;",
                    '<' => "&lt;",
                    '&' => "&amp;",
                    '\'' => "&#39;",
                    '"' => "&quot;",
                    _ => unreachable!(),
                };
                buf.write_str(s)?;
                last = i + 1;
            }
            _ => {}
        }
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
                escape(&line[cur_index..i], &mut s)?;
            } else {
                escape(&line[cur_index..i], &mut s)?;
            }
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
                if span_empty {
                    s.truncate(span_start);
                } else {
                    s.push_str("</span>");
                }
                span_delta -= 1;
                span_empty = false;

                if handling_link {
                    s.push_str("</a>");
                    handling_link = false;
                }
            }
        })?;
    }
    escape(&line[cur_index..line.len()], &mut s)?;
    Ok((s, span_delta))
}

impl Highlighter {
    fn highlight_inner(&self, source: &str, ext: Option<&str>) -> Result<String, Error> {
        let syntax_ref = self
            .syntax_set
            .find_syntax_by_extension(ext.unwrap_or("txt"))
            .unwrap_or_else(|| {
                self.syntax_set
                    .find_syntax_by_extension("txt")
                    .expect("finding txt syntax")
            });

        let is_markdown = syntax_ref.name == "Markdown";
        let mut parse_state = ParseState::new(syntax_ref);
        let mut html = String::from("<table><tbody>");
        let mut scope_stack = ScopeStack::new();

        for (mut line_number, line) in LinesWithEndings::from(source).enumerate() {
            let (formatted, delta) = if line.len() > HIGHLIGHT_LINE_LENGTH_CUTOFF {
                (line.to_string(), 0)
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

    /// Highlight `data` with the given file extension.
    pub async fn highlight(&self, text: String, ext: Option<String>) -> Result<Html, Error> {
        let highlighter = self.clone();

        Ok(Html(
            tokio::task::spawn_blocking(move || highlighter.highlight_inner(&text, ext.as_deref()))
                .await??,
        ))
    }
}

impl Html {
    pub fn into_inner(self) -> String {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_links() -> Result<(), Box<dyn std::error::Error>> {
        let highlighter = Highlighter::default();
        let html = highlighter
            .highlight_inner("[hello](https://github.com/matze/wastebin)", Some("md"))?;

        assert!(html.contains("<span class=\"markup underline link markdown\"><a href=\"https://github.com/matze/wastebin\">https://github.com/matze/wastebin</span></a>"));

        Ok(())
    }
}
