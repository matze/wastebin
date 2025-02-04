use crate::db::read::Entry;
use crate::errors::Error;
use std::cmp::Ordering;
use syntect::html::{line_tokens_to_classed_spans, ClassStyle};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

const HIGHLIGHT_LINE_LENGTH_CUTOFF: usize = 2048;

/// Supported themes.
#[derive(Copy, Clone)]
pub enum Theme {
    Ayu,
    Base16Ocean,
    Coldark,
    Gruvbox,
    Monokai,
    Onehalf,
    Solarized,
}

#[derive(Clone)]
pub struct Html(String);

#[derive(Clone)]
pub struct Highlighter {
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

impl Highlighter {
    fn highlight_inner(&self, source: &str, ext: &str) -> Result<String, Error> {
        let syntax_ref = self
            .syntax_set
            .find_syntax_by_extension(ext)
            .unwrap_or_else(|| {
                self.syntax_set
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
                let parsed = parse_state.parse_line(line, &self.syntax_set)?;
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

    /// Highlight `entry` with the given file extension.
    pub async fn highlight(&self, entry: Entry, ext: String) -> Result<Html, Error> {
        let highlighter = self.clone();

        Ok(Html(
            tokio::task::spawn_blocking(move || highlighter.highlight_inner(&entry.text, &ext))
                .await??,
        ))
    }
}

impl Html {
    pub fn into_inner(self) -> String {
        self.0
    }
}
