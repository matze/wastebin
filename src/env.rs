use crate::{db, highlight};
use axum_extra::extract::cookie::Key;
use once_cell::sync::Lazy;
use std::env::VarError;
use std::net::SocketAddr;
use std::num::{NonZeroUsize, ParseIntError};
use std::path::PathBuf;
use std::string::String;
use std::time::Duration;

pub struct Metadata<'a> {
    pub title: String,
    pub version: &'a str,
    pub highlight: &'a highlight::Data<'a>,
}

pub static METADATA: Lazy<Metadata> = Lazy::new(|| {
    let title = std::env::var("WASTEBIN_TITLE").unwrap_or_else(|_| "wastebin".to_string());
    let version = env!("CARGO_PKG_VERSION");
    let highlight = &highlight::DATA;

    Metadata {
        title,
        version,
        highlight,
    }
});

pub const HTTP_TIMEOUT: Duration = Duration::from_secs(5);

pub const CSS_MAX_AGE: Duration = Duration::from_secs(60 * 60 * 24 * 30 * 6);

pub const FAVICON_MAX_AGE: Duration = Duration::from_secs(86400);

const VAR_ADDRESS_PORT: &str = "WASTEBIN_ADDRESS_PORT";
const VAR_CACHE_SIZE: &str = "WASTEBIN_CACHE_SIZE";
const VAR_DATABASE_PATH: &str = "WASTEBIN_DATABASE_PATH";
const VAR_MAX_BODY_SIZE: &str = "WASTEBIN_MAX_BODY_SIZE";
const VAR_SIGNING_KEY: &str = "WASTEBIN_SIGNING_KEY";

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
    #[error("failed to generate key from {VAR_SIGNING_KEY}: {0}")]
    SigningKey(String),
}

pub fn cache_size() -> Result<NonZeroUsize, Error> {
    std::env::var(VAR_CACHE_SIZE)
        .map_or_else(
            |_| Ok(NonZeroUsize::new(128).unwrap()),
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
