use crate::env;
use crate::highlight;
use askama::Template;
use axum::http::StatusCode;
use std::default::Default;

/// Error page showing a message.
#[derive(Template)]
#[template(path = "error.html")]
pub struct Error<'a> {
    meta: &'a env::Metadata<'a>,
    error: String,
}

/// Error response carrying a status code and the page itself.
pub type ErrorResponse<'a> = (StatusCode, Error<'a>);

impl From<crate::Error> for ErrorResponse<'_> {
    fn from(err: crate::Error) -> Self {
        let html = Error {
            meta: &env::METADATA,
            error: err.to_string(),
        };

        (err.into(), html)
    }
}

/// Index page displaying a form for paste insertion and a selection box for languages.
#[derive(Template)]
#[template(path = "index.html")]
pub struct Index<'a> {
    meta: &'a env::Metadata<'a>,
    syntaxes: &'a [syntect::parsing::SyntaxReference],
}

impl<'a> Default for Index<'a> {
    fn default() -> Self {
        Self {
            meta: &env::METADATA,
            syntaxes: highlight::DATA.syntaxes(),
        }
    }
}

/// Paste view showing the formatted paste as well as a bunch of links.
#[derive(Template)]
#[template(path = "paste.html")]
pub struct Paste<'a> {
    meta: &'a env::Metadata<'a>,
    id: String,
    html: String,
    ext: String,
    can_delete: bool,
}

impl<'a> Paste<'a> {
    /// Construct new paste view from cache `entry` and cache `key`.
    pub fn new(id: String, ext: String, html: String, can_delete: bool) -> Self {
        Self {
            meta: &env::METADATA,
            id,
            ext,
            html,
            can_delete,
        }
    }
}

/// Burn page shown if "burn-after-reading" was selected during insertion.
#[derive(Template)]
#[template(path = "burn.html")]
pub struct Burn<'a> {
    meta: &'a env::Metadata<'a>,
    id: String,
}

impl<'a> Burn<'a> {
    /// Construct new burn page linking to `id`.
    pub fn new(id: String) -> Self {
        Self {
            meta: &env::METADATA,
            id,
        }
    }
}
