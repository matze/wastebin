use std::cell::Cell;

use crate::cache::Key as CacheKey;
use crate::env;
use crate::highlight::Html;
use crate::routes::paste::{Format, QueryData};
use askama::Template;
use axum::http::StatusCode;

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
    max_expiry: Option<u32>,

    /// SAFETY: calls in the template are always sequential
    default_has_been_written: Cell<bool>,
}

impl<'a> Default for Index<'a> {
    fn default() -> Self {
        Self {
            meta: env::metadata(),
            base_path: env::base_path(),
            max_expiry: env::max_paste_expiry(),
            default_has_been_written: Cell::new(false),
        }
    }
}

#[derive(Debug)]
enum Expiry<'a> {
    Special(&'a str),
    Time(u32),
}

impl<'a> Expiry<'a> {
    fn as_str(&self) -> String {
        match self {
            Expiry::Special(s) => s.to_string(),
            Expiry::Time(u) => u.to_string(),
        }
    }
}

impl<'a> Index<'a> {
    fn expiry(&self, name: &str, time: Expiry<'a>) -> String {
        let sel_string = if self.default_has_been_written.get() {
            ""
        } else {
            r#" selected"#
        };

        match self.max_expiry {
            Some(exp) => {
                match time {
                    // never emit an never expire with a limit
                    Expiry::Special("") => String::new(),
                    Expiry::Special("burn") => {
                        self.default_has_been_written.set(true);
                        format!(r#"<option{} value="burn">{}</option>"#, sel_string, name)
                    }
                    Expiry::Special(_) => String::new(),
                    Expiry::Time(t) if t > exp => String::new(),
                    Expiry::Time(t) => {
                        self.default_has_been_written.set(true);
                        format!(r#"<option{} value="{}">{}</option>"#, sel_string, t, name)
                    }
                }
            }
            None => {
                self.default_has_been_written.set(true);
                format!(
                    r#"<option{} value="{}">{}</option>"#,
                    sel_string, time, name
                )
            }
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
