use crate::cache::Key as CacheKey;
use crate::env;
use crate::highlight::Html;
use crate::routes::paste::{Format, QueryData};
use askama::Template;
use axum::http::StatusCode;
use std::default::Default;

/// Error page showing a message.
#[derive(Template)]
#[template(path = "error.html")]
pub struct Error<'a> {
    meta: &'a env::Metadata<'a>,
    base_path: &'static env::BasePath,
    description: String,
}

/// Error response carrying a status code and the page itself.
pub type ErrorResponse<'a> = (StatusCode, Error<'a>);

impl From<crate::Error> for ErrorResponse<'_> {
    fn from(err: crate::Error) -> Self {
        let html = Error {
            meta: env::metadata(),
            base_path: env::base_path(),
            description: err.to_string(),
        };

        (err.into(), html)
    }
}

/// Index page displaying a form for paste insertion and a selection box for languages.
#[derive(Template)]
#[template(path = "index.html")]
pub struct Index<'a> {
    meta: &'a env::Metadata<'a>,
    base_path: &'static env::BasePath,
}

impl<'a> Default for Index<'a> {
    fn default() -> Self {
        Self {
            meta: env::metadata(),
            base_path: env::base_path(),
        }
    }
}

/// Paste view showing the formatted paste as well as a bunch of links.
#[derive(Template)]
#[template(path = "formatted.html")]
pub struct Paste<'a> {
    meta: &'a env::Metadata<'a>,
    base_path: &'static env::BasePath,
    id: String,
    ext: String,
    can_delete: bool,
    html: String,
}

impl<'a> Paste<'a> {
    /// Construct new paste view from cache `key` and paste `html`.
    pub fn new(key: CacheKey, html: Html, can_delete: bool) -> Self {
        let html = html.into_inner();

        Self {
            meta: env::metadata(),
            base_path: env::base_path(),
            id: key.id(),
            ext: key.ext,
            can_delete,
            html,
        }
    }
}

/// View showing password input.
#[derive(Template)]
#[template(path = "encrypted.html")]
pub struct Encrypted<'a> {
    meta: &'a env::Metadata<'a>,
    base_path: &'static env::BasePath,
    id: String,
    ext: String,
    query: String,
}

impl<'a> Encrypted<'a> {
    /// Construct new paste view from cache `key` and paste `html`.
    pub fn new(key: CacheKey, query: QueryData) -> Self {
        let query = match (query.fmt, query.dl) {
            (Some(Format::Raw), None) => "?fmt=raw".to_string(),
            (Some(Format::Qr), None) => "?fmt=qr".to_string(),
            (None, Some(dl)) => format!("?dl={dl}"),
            _ => String::new(),
        };

        Self {
            meta: env::metadata(),
            base_path: env::base_path(),
            id: key.id(),
            ext: key.ext,
            query,
        }
    }
}

/// Paste view showing the formatted paste as well as a bunch of links.
#[derive(Template)]
#[template(path = "qr.html", escape = "none")]
pub struct Qr<'a> {
    meta: &'a env::Metadata<'a>,
    base_path: &'static env::BasePath,
    id: String,
    ext: String,
    can_delete: bool,
    code: qrcodegen::QrCode,
}

impl<'a> Qr<'a> {
    /// Construct new QR code view from `code`.
    pub fn new(code: qrcodegen::QrCode, key: CacheKey) -> Self {
        Self {
            meta: env::metadata(),
            base_path: env::base_path(),
            id: key.id(),
            ext: key.ext,
            code,
            can_delete: false,
        }
    }

    // Return module coordinates that are dark.
    fn dark_modules(&self) -> Vec<(i32, i32)> {
        let size = self.code.size();
        (0..size)
            .flat_map(|x| (0..size).map(move |y| (x, y)))
            .filter(|(x, y)| self.code.get_module(*x, *y))
            .collect()
    }
}

/// Burn page shown if "burn-after-reading" was selected during insertion.
#[derive(Template)]
#[template(path = "burn.html")]
pub struct Burn<'a> {
    meta: &'a env::Metadata<'a>,
    base_path: &'static env::BasePath,
    id: String,
}

impl<'a> Burn<'a> {
    /// Construct new burn page linking to `id`.
    pub fn new(id: String) -> Self {
        Self {
            meta: env::metadata(),
            base_path: env::base_path(),
            id,
        }
    }
}
