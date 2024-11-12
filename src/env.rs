use crate::{db, highlight};
use axum_extra::extract::cookie::Key;
use std::env::VarError;
use std::net::SocketAddr;
use std::num::{NonZero, NonZeroU32, NonZeroUsize, ParseIntError};
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration;

pub struct Metadata<'a> {
    pub title: String,
    pub version: &'a str,
    pub highlight: &'a highlight::Data<'a>,
}

pub const DEFAULT_HTTP_TIMEOUT: Duration = Duration::from_secs(5);

pub const CSS_MAX_AGE: Duration = Duration::from_secs(60 * 60 * 24 * 30 * 6); // 6 month
pub const JS_MAX_AGE: Duration = Duration::from_secs(60 * 60 * 24 * 30 * 6); // 6 month

pub const FAVICON_MAX_AGE: Duration = Duration::from_secs(86400);

const VAR_ADDRESS_PORT: &str = "WASTEBIN_ADDRESS_PORT";
const VAR_CACHE_SIZE: &str = "WASTEBIN_CACHE_SIZE";
const VAR_DATABASE_PATH: &str = "WASTEBIN_DATABASE_PATH";
const VAR_MAX_BODY_SIZE: &str = "WASTEBIN_MAX_BODY_SIZE";
const VAR_SIGNING_KEY: &str = "WASTEBIN_SIGNING_KEY";
const VAR_BASE_URL: &str = "WASTEBIN_BASE_URL";
const VAR_PASSWORD_SALT: &str = "WASTEBIN_PASSWORD_SALT";
const VAR_HTTP_TIMEOUT: &str = "WASTEBIN_HTTP_TIMEOUT";
const VAR_MAX_PASTE_EXPIRATION: &str = "WASTEBIN_MAX_PASTE_EXPIRATION";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to parse {VAR_CACHE_SIZE}, expected number of elements: {0}")]
    CacheSize(ParseIntError),
    #[error("failed to parse {VAR_DATABASE_PATH}, contains non-Unicode data")]
    DatabasePath,
    #[error("failed to parse {VAR_MAX_BODY_SIZE}, expected number of bytes: {0}")]
    MaxBodySize(ParseIntError),
    #[error("failed to parse {VAR_ADDRESS_PORT}, expected `host:port`")]
    AddressPort,
    #[error("failed to parse {VAR_BASE_URL}: {0}")]
    BaseUrl(String),
    #[error("failed to generate key from {VAR_SIGNING_KEY}: {0}")]
    SigningKey(String),
    #[error("failed to parse {VAR_HTTP_TIMEOUT}: {0}")]
    HttpTimeout(ParseIntError),
    #[error("failed to parse {VAR_MAX_PASTE_EXPIRATION}: {0}")]
    MaxPasteExpiration(ParseIntError),
}

pub struct BasePath(String);

impl BasePath {
    pub fn path(&self) -> &str {
        &self.0
    }

    pub fn join(&self, s: &str) -> String {
        let b = &self.0;
        format!("{b}{s}")
    }
}

impl Default for BasePath {
    fn default() -> Self {
        BasePath("/".to_string())
    }
}

pub static METADATA: LazyLock<Metadata> = LazyLock::new(|| {
    let title = std::env::var("WASTEBIN_TITLE").unwrap_or_else(|_| "wastebin".to_string());
    let version = env!("CARGO_PKG_VERSION");
    let highlight = &highlight::DATA;

    Metadata {
        title,
        version,
        highlight,
    }
});

// NOTE: This relies on `VAR_BASE_URL` but repeats parsing to handle errors.
pub static BASE_PATH: LazyLock<BasePath> = LazyLock::new(|| {
    std::env::var(VAR_BASE_URL).map_or_else(
        |err| {
            match err {
                VarError::NotPresent => (),
                VarError::NotUnicode(_) => {
                    tracing::warn!("`VAR_BASE_URL` not Unicode, defaulting to '/'");
                }
            };
            BasePath::default()
        },
        |var| match url::Url::parse(&var) {
            Ok(url) => {
                let path = url.path();

                if path.ends_with('/') {
                    BasePath(path.to_string())
                } else {
                    BasePath(format!("{path}/"))
                }
            }
            Err(err) => {
                tracing::error!("error parsing `VAR_BASE_URL`, defaulting to '/': {err}");
                BasePath::default()
            }
        },
    )
});

pub fn cache_size() -> Result<NonZeroUsize, Error> {
    std::env::var(VAR_CACHE_SIZE)
        .map_or_else(
            |_| Ok(NonZeroUsize::new(128).expect("128 is non-zero")),
            |s| s.parse::<NonZeroUsize>(),
        )
        .map_err(Error::CacheSize)
}

pub fn database_method() -> Result<db::Open, Error> {
    match std::env::var(VAR_DATABASE_PATH) {
        Ok(path) => Ok(db::Open::Path(PathBuf::from(path))),
        Err(VarError::NotUnicode(_)) => Err(Error::DatabasePath),
        Err(VarError::NotPresent) => Ok(db::Open::Memory),
    }
}

pub fn signing_key() -> Result<Key, Error> {
    std::env::var(VAR_SIGNING_KEY).map_or_else(
        |_| Ok(Key::generate()),
        |s| Key::try_from(s.as_bytes()).map_err(|err| Error::SigningKey(err.to_string())),
    )
}

pub fn addr() -> Result<SocketAddr, Error> {
    std::env::var(VAR_ADDRESS_PORT)
        .as_ref()
        .map(String::as_str)
        .unwrap_or("0.0.0.0:8088")
        .parse()
        .map_err(|_| Error::AddressPort)
}

pub fn max_body_size() -> Result<usize, Error> {
    std::env::var(VAR_MAX_BODY_SIZE)
        .map_or_else(|_| Ok(1024 * 1024), |s| s.parse::<usize>())
        .map_err(Error::MaxBodySize)
}

pub fn base_url() -> Result<Option<url::Url>, Error> {
    let result = std::env::var(VAR_BASE_URL).map_or_else(
        |err| match err {
            VarError::NotPresent => Ok(None),
            VarError::NotUnicode(_) => Err(Error::BaseUrl("Not Unicode".to_string())),
        },
        |var| {
            Ok(Some(
                url::Url::parse(&var).map_err(|err| Error::BaseUrl(err.to_string()))?,
            ))
        },
    )?;

    Ok(result)
}

pub fn password_hash_salt() -> String {
    std::env::var(VAR_PASSWORD_SALT).unwrap_or_else(|_| "somesalt".to_string())
}

pub fn http_timeout() -> Result<Duration, Error> {
    std::env::var(VAR_HTTP_TIMEOUT)
        .map_or_else(
            |_| Ok(DEFAULT_HTTP_TIMEOUT),
            |s| s.parse::<u64>().map(|v| Duration::new(v, 0)),
        )
        .map_err(Error::HttpTimeout)
}

pub fn max_paste_expiration() -> Result<Option<NonZeroU32>, Error> {
    std::env::var(VAR_MAX_PASTE_EXPIRATION)
        .ok()
        .map(|value| value.parse::<u32>().map_err(Error::MaxPasteExpiration))
        .transpose()
        .map(|op| op.and_then(NonZero::new))
}
