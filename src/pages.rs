use crate::cache::Key as CacheKey;
use crate::highlight::{Highlighter, Html};
use crate::routes::paste::{Format, QueryData};
use crate::{errors, Page};
use askama::Template;
use axum::http::StatusCode;
use std::num::{NonZero, NonZeroU32};
use std::sync::{Arc, OnceLock};

/// Error page showing a message.
#[derive(Template)]
#[template(path = "error.html")]
pub struct Error {
    page: Arc<Page>,
    description: String,
}

/// Error response carrying a status code and the page itself.
pub type ErrorResponse = (StatusCode, Error);

/// Create an error response from `error` consisting of [`StatusCode`] derive from `error` as well
/// as a rendered page with a description.
pub fn make_error(error: errors::Error, page: Arc<Page>) -> ErrorResponse {
    let description = error.to_string();
    (error.into(), Error { page, description })
}

impl Error {
    /// Create new [`Error`] from `description`.
    pub fn new(description: String, page: Arc<Page>) -> Self {
        Self { page, description }
    }
}

/// Index page displaying a form for paste insertion and a selection box for languages.
#[derive(Template)]
#[template(path = "index.html")]
pub struct Index {
    page: Arc<Page>,
    max_expiration: Option<NonZeroU32>,
    highlighter: Arc<Highlighter>,
}

impl Index {
    pub fn new(
        max_expiration: Option<NonZeroU32>,
        page: Arc<Page>,
        highlighter: Arc<Highlighter>,
    ) -> Self {
        Self {
            page,
            max_expiration,
            highlighter,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Expiration {
    None,
    Burn,
    Time(NonZeroU32),
}

impl std::fmt::Display for Expiration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expiration::None => write!(f, ""),
            Expiration::Burn => write!(f, "burn"),
            Expiration::Time(t) => write!(f, "{t}"),
        }
    }
}

#[allow(clippy::unwrap_used)]
const EXPIRATION_OPTIONS: [(&str, Expiration); 8] = [
    ("never", Expiration::None),
    ("10 minutes", Expiration::Time(NonZero::new(600).unwrap())),
    ("1 hour", Expiration::Time(NonZero::new(3600).unwrap())),
    ("1 day", Expiration::Time(NonZero::new(86400).unwrap())),
    ("1 week", Expiration::Time(NonZero::new(604_800).unwrap())),
    (
        "1 month",
        Expiration::Time(NonZero::new(2_592_000).unwrap()),
    ),
    (
        "1 year",
        Expiration::Time(NonZero::new(31_536_000).unwrap()),
    ),
    ("🔥 after reading", Expiration::Burn),
];

impl Index {
    fn expiry_options(&self) -> &str {
        static EXPIRATION_OPTIONS_HTML: OnceLock<String> = OnceLock::new();

        EXPIRATION_OPTIONS_HTML.get_or_init(|| {

            let mut option_set = String::new();
            let mut wrote_first = false;

            option_set.push('\n');

            for (opt_name, opt_val) in EXPIRATION_OPTIONS {
                if self.max_expiration.is_none()
                    || opt_val == Expiration::Burn
                    || matches!((self.max_expiration, opt_val), (Some(exp), Expiration::Time(time)) if time <= exp)
                {
                    option_set.push_str("<option");
                    if !wrote_first {
                        option_set.push_str(" selected");
                        wrote_first = true;
                    }
                    option_set.push_str(" value=\"");
                    option_set.push_str(opt_val.to_string().as_ref());
                    option_set.push_str("\">");
                    option_set.push_str(opt_name);
                    option_set.push_str("</option>\n");
                }
            }

            option_set
        })
    }
}

/// Paste view showing the formatted paste as well as a bunch of links.
#[derive(Template)]
#[template(path = "formatted.html")]
pub struct Paste {
    page: Arc<Page>,
    id: String,
    ext: String,
    can_delete: bool,
    html: String,
    title: String,
}

impl Paste {
    /// Construct new paste view from cache `key` and paste `html`.
    pub fn new(
        key: CacheKey,
        html: Html,
        can_delete: bool,
        title: String,
        page: Arc<Page>,
    ) -> Self {
        let html = html.into_inner();

        Self {
            page,
            id: key.id(),
            ext: key.ext,
            can_delete,
            html,
            title,
        }
    }
}

/// View showing password input.
#[derive(Template)]
#[template(path = "encrypted.html")]
pub struct Encrypted {
    page: Arc<Page>,
    id: String,
    ext: String,
    query: String,
}

impl Encrypted {
    /// Construct new paste view from cache `key` and paste `html`.
    pub fn new(key: CacheKey, query: &QueryData, page: Arc<Page>) -> Self {
        let query = match query.fmt {
            Some(Format::Raw) => "?fmt=raw".to_string(),
            Some(Format::Qr) => "?fmt=qr".to_string(),
            Some(Format::Dl) => "?fmt=dl".to_string(),
            None => String::new(),
        };

        Self {
            page,
            id: key.id(),
            ext: key.ext,
            query,
        }
    }
}

/// Return module coordinates that are dark.
fn dark_modules(code: &qrcodegen::QrCode) -> Vec<(i32, i32)> {
    let size = code.size();
    (0..size)
        .flat_map(|x| (0..size).map(move |y| (x, y)))
        .filter(|(x, y)| code.get_module(*x, *y))
        .collect()
}

/// Paste view showing the formatted paste as well as a bunch of links.
#[derive(Template)]
#[template(path = "qr.html", escape = "none")]
pub struct Qr {
    page: Arc<Page>,
    id: String,
    ext: String,
    can_delete: bool,
    code: qrcodegen::QrCode,
    title: String,
}

impl Qr {
    /// Construct new QR code view from `code`.
    pub fn new(code: qrcodegen::QrCode, key: CacheKey, title: String, page: Arc<Page>) -> Self {
        Self {
            page,
            id: key.id(),
            ext: key.ext,
            code,
            can_delete: false,
            title,
        }
    }

    fn dark_modules(&self) -> Vec<(i32, i32)> {
        dark_modules(&self.code)
    }
}

/// Burn page shown if "burn-after-reading" was selected during insertion.
#[derive(Template)]
#[template(path = "burn.html", escape = "none")]
pub struct Burn {
    page: Arc<Page>,
    id: String,
    code: qrcodegen::QrCode,
}

impl Burn {
    /// Construct new burn page linking to `id`.
    pub fn new(code: qrcodegen::QrCode, id: String, page: Arc<Page>) -> Self {
        Self { page, id, code }
    }

    fn dark_modules(&self) -> Vec<(i32, i32)> {
        dark_modules(&self.code)
    }
}
