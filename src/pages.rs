use crate::db::CacheKey;
use crate::highlight;
use askama::Template;
use axum::http::StatusCode;
use std::default::Default;

/// Error page showing a message.
#[derive(Template)]
#[template(path = "error.html")]
pub struct Error<'a> {
    title: &'a str,
    error: String,
    version: &'a str,
}

/// Error response carrying a status code and the page itself.
pub type ErrorResponse<'a> = (StatusCode, Error<'a>);

impl From<crate::Error> for ErrorResponse<'_> {
    fn from(err: crate::Error) -> Self {
        let html = Error {
            title: &crate::TITLE,
            error: err.to_string(),
            version: crate::VERSION,
        };

        (err.into(), html)
    }
}

/// Index page displaying a form for paste insertion and a selection box for languages.
#[derive(Template)]
#[template(path = "index.html")]
pub struct Index<'a> {
    title: &'a str,
    syntaxes: &'a [syntect::parsing::SyntaxReference],
    version: &'a str,
}

impl<'a> Default for Index<'a> {
    fn default() -> Self {
        Self {
            title: &crate::TITLE,
            syntaxes: highlight::DATA.syntax_set.syntaxes(),
            version: crate::VERSION,
        }
    }
}

/// Paste view showing the formatted paste as well as a bunch of links.
#[derive(Template)]
#[template(path = "paste.html")]
pub struct Paste<'a> {
    title: &'a str,
    id: String,
    html: String,
    extension: String,
    can_delete: bool,
    version: &'a str,
}

impl<'a> Paste<'a> {
    /// Construct new paste view from cache `entry` and cache `key`.
    pub fn new(html: String, key: CacheKey, can_delete: bool) -> Self {
        Self {
            title: &crate::TITLE,
            id: key.id(),
            extension: key.ext,
            html,
            can_delete,
            version: crate::VERSION,
        }
    }
}

/// Burn page shown if "burn-after-reading" was selected during insertion.
#[derive(Template)]
#[template(path = "burn.html")]
pub struct Burn<'a> {
    title: &'a str,
    id: String,
    version: &'a str,
}

impl<'a> Burn<'a> {
    /// Construct new burn page linking to `id`.
    pub fn new(id: String) -> Self {
        Self {
            title: &crate::TITLE,
            id,
            version: crate::VERSION,
        }
    }
}
