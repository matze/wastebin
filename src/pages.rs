use crate::{db, env};
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
}

impl<'a> Default for Index<'a> {
    fn default() -> Self {
        Self {
            meta: &env::METADATA,
        }
    }
}

/// Paste view showing the formatted paste as well as a bunch of links.
#[derive(Template)]
#[template(path = "formatted.html")]
pub struct Paste<'a> {
    meta: &'a env::Metadata<'a>,
    id: String,
    ext: String,
    can_delete: bool,
    html: String,
}

impl<'a> Paste<'a> {
    /// Construct new paste view from cache `key` and paste `html`.
    pub fn new(key: db::CacheKey, html: String, can_delete: bool) -> Self {
        Self {
            meta: &env::METADATA,
            id: key.id(),
            ext: key.ext,
            can_delete,
            html,
        }
    }
}

/// Paste view showing the formatted paste as well as a bunch of links.
#[derive(Template)]
#[template(path = "qr.html", escape = "none")]
pub struct Qr<'a> {
    meta: &'a env::Metadata<'a>,
    id: String,
    ext: String,
    can_delete: bool,
    qr: qrcodegen::QrCode,
}

impl<'a> Qr<'a> {
    /// Construct new QR code view from `code`.
    pub fn new(qr: qrcodegen::QrCode, key: db::CacheKey) -> Self {
        Self {
            meta: &env::METADATA,
            id: key.id(),
            ext: key.ext,
            qr,
            can_delete: false,
        }
    }

    // Return module coordinates that are dark.
    fn dark_modules(&self) -> Vec<(i32, i32)> {
        let size = self.qr.size();
        (0..size)
            .flat_map(|x| (0..size).map(move |y| (x, y)))
            .filter(|(x, y)| self.qr.get_module(*x, *y))
            .collect()
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
