use crate::{expiration, highlight};
use axum_extra::extract::cookie::Key;
use std::env::VarError;
use std::net::SocketAddr;
use std::num::{NonZero, NonZeroU32, NonZeroUsize, ParseIntError};
use std::path::PathBuf;
use std::time::Duration;
use wastebin_core::db;
use wastebin_core::env::vars::{
    self, ADDRESS_PORT, BASE_URL, CACHE_SIZE, DATABASE_PATH, HTTP_TIMEOUT, MAX_BODY_SIZE,
    PASTE_EXPIRATIONS, RATELIMIT_DELETE, RATELIMIT_INSERT, SIGNING_KEY,
};

pub const DEFAULT_HTTP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {
    #[error("failed to parse {CACHE_SIZE}, expected number of elements: {0}")]
    CacheSize(ParseIntError),
    #[error("failed to parse {DATABASE_PATH}, contains non-Unicode data")]
    DatabasePath,
    #[error("failed to parse {MAX_BODY_SIZE}, expected number of bytes: {0}")]
    MaxBodySize(ParseIntError),
    #[error("failed to parse {ADDRESS_PORT}, expected `host:port`")]
    AddressPort,
    #[error("failed to parse {BASE_URL}: {0}")]
    BaseUrl(String),
    #[error("failed to generate key from {SIGNING_KEY}: {0}")]
    SigningKey(String),
    #[error("failed to parse {HTTP_TIMEOUT}: {0}")]
    HttpTimeout(ParseIntError),
    #[error("failed to parse {PASTE_EXPIRATIONS}: {0}")]
    ParsePasteExpiration(#[from] expiration::Error),
    #[error("unknown theme {0}")]
    UnknownTheme(String),
    #[error("failed to parse {RATELIMIT_INSERT}: {0}")]
    RatelimitInsert(ParseIntError),
    #[error("failed to parse {RATELIMIT_DELETE}: {0}")]
    RatelimitDelete(ParseIntError),
}

pub fn title() -> String {
    std::env::var(vars::TITLE).unwrap_or_else(|_| "wastebin".to_string())
}

pub fn theme() -> Result<highlight::Theme, Error> {
    std::env::var(vars::THEME).map_or_else(
        |_| Ok(highlight::Theme::Ayu),
        |var| match var.as_str() {
            "ayu" => Ok(highlight::Theme::Ayu),
            "base16ocean" => Ok(highlight::Theme::Base16Ocean),
            "coldark" => Ok(highlight::Theme::Coldark),
            "gruvbox" => Ok(highlight::Theme::Gruvbox),
            "monokai" => Ok(highlight::Theme::Monokai),
            "onehalf" => Ok(highlight::Theme::Onehalf),
            "solarized" => Ok(highlight::Theme::Solarized),
            _ => Err(Error::UnknownTheme(var)),
        },
    )
}

pub fn cache_size() -> Result<NonZeroUsize, Error> {
    std::env::var(vars::CACHE_SIZE)
        .map_or_else(
            |_| Ok(NonZeroUsize::new(128).expect("128 is non-zero")),
            |s| s.parse::<NonZeroUsize>(),
        )
        .map_err(Error::CacheSize)
}

pub fn database_method() -> Result<db::Open, Error> {
    match std::env::var(vars::DATABASE_PATH) {
        Ok(path) => Ok(db::Open::Path(PathBuf::from(path))),
        Err(VarError::NotUnicode(_)) => Err(Error::DatabasePath),
        Err(VarError::NotPresent) => Ok(db::Open::Memory),
    }
}

pub fn signing_key() -> Result<Key, Error> {
    std::env::var(vars::SIGNING_KEY).map_or_else(
        |_| Ok(Key::generate()),
        |s| Key::try_from(s.as_bytes()).map_err(|err| Error::SigningKey(err.to_string())),
    )
}

pub fn addr() -> Result<SocketAddr, Error> {
    std::env::var(vars::ADDRESS_PORT)
        .as_ref()
        .map(String::as_str)
        .unwrap_or("0.0.0.0:8088")
        .parse()
        .map_err(|_| Error::AddressPort)
}

pub fn max_body_size() -> Result<usize, Error> {
    std::env::var(vars::MAX_BODY_SIZE)
        .map_or_else(|_| Ok(1024 * 1024), |s| s.parse::<usize>())
        .map_err(Error::MaxBodySize)
}

/// Read base URL either from the environment variable or fallback to the hostname.
pub fn base_url() -> Result<url::Url, Error> {
    if let Some(base_url) = std::env::var(vars::BASE_URL).map_or_else(
        |err| {
            if matches!(err, VarError::NotUnicode(_)) {
                Err(Error::BaseUrl(format!("{BASE_URL} is not unicode")))
            } else {
                Ok(None)
            }
        },
        |var| {
            Ok(Some(
                url::Url::parse(&var).map_err(|err| Error::BaseUrl(err.to_string()))?,
            ))
        },
    )? {
        return Ok(base_url);
    }

    let hostname =
        hostname::get().map_err(|err| Error::BaseUrl(format!("failed to get hostname: {err}")))?;

    url::Url::parse(&format!("https://{}", hostname.to_string_lossy()))
        .map_err(|err| Error::BaseUrl(err.to_string()))
}

pub fn http_timeout() -> Result<Duration, Error> {
    std::env::var(vars::HTTP_TIMEOUT)
        .map_or_else(
            |_| Ok(DEFAULT_HTTP_TIMEOUT),
            |s| s.parse::<u64>().map(|v| Duration::new(v, 0)),
        )
        .map_err(Error::HttpTimeout)
}

/// Parse [`expiration::ExpirationSet`] from environment or return default.
pub fn expiration_set() -> Result<expiration::ExpirationSet, Error> {
    let set = std::env::var(vars::PASTE_EXPIRATIONS).map_or_else(
        |_| "0,600,3600=d,86400,604800,2419200,29030400".parse::<expiration::ExpirationSet>(),
        |value| value.parse::<expiration::ExpirationSet>(),
    )?;

    Ok(set)
}

pub fn ratelimit_insert() -> Result<Option<NonZeroU32>, Error> {
    std::env::var(vars::RATELIMIT_INSERT)
        .ok()
        .map(|value| value.parse::<u32>().map_err(Error::RatelimitInsert))
        .transpose()
        .map(|op| op.and_then(NonZero::new))
}

pub fn ratelimit_delete() -> Result<Option<NonZeroU32>, Error> {
    std::env::var(vars::RATELIMIT_DELETE)
        .ok()
        .map(|value| value.parse::<u32>().map_err(Error::RatelimitDelete))
        .transpose()
        .map(|op| op.and_then(NonZero::new))
}
