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
            meta: &env::METADATA,
            base_path: &env::BASE_PATH,
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
    max_expiration: Option<u32>,
}

impl<'a> Default for Index<'a> {
    fn default() -> Self {
        Self {
            meta: &env::METADATA,
            base_path: &env::BASE_PATH,
            // exception should already have been handled in main
            max_expiration: env::max_paste_expiration().expect("parsing max paste expiration"),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Expiration {
    None,
    Burn,
    Time(u32),
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

const EXPIRATION_OPTIONS: [(&str, Expiration); 8] = [
    ("never", Expiration::None),
    ("10 minutes", Expiration::Time(600)),
    ("1 hour", Expiration::Time(3600)),
    ("1 day", Expiration::Time(86400)),
    ("1 week", Expiration::Time(604_800)),
    ("1 month", Expiration::Time(2_592_000)),
    ("1 year", Expiration::Time(31_536_000)),
    ("🔥 after reading", Expiration::Burn),
];

impl<'a> Index<'a> {
    fn expiry_options(&self) -> String {
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
            meta: &env::METADATA,
            base_path: &env::BASE_PATH,
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
            meta: &env::METADATA,
            base_path: &env::BASE_PATH,
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
            meta: &env::METADATA,
            base_path: &env::BASE_PATH,
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
            meta: &env::METADATA,
            base_path: &env::BASE_PATH,
            id,
        }
    }
}
