use crate::db::Database;
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::{Router, Server};
use axum_extra::extract::cookie::Key;
use once_cell::sync::Lazy;
use std::env::{self, VarError};
use std::net::SocketAddr;
use std::num::{NonZeroUsize, ParseIntError, TryFromIntError};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

mod cache;
mod db;
mod handler;
mod highlight;
mod id;
mod pages;
#[cfg(test)]
mod test_helpers;

pub static TITLE: Lazy<String> =
    Lazy::new(|| env::var("WASTEBIN_TITLE").unwrap_or_else(|_| "wastebin".to_string()));

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const VAR_ADDRESS_PORT: &str = "WASTEBIN_ADDRESS_PORT";
const VAR_CACHE_SIZE: &str = "WASTEBIN_CACHE_SIZE";
const VAR_DATABASE_PATH: &str = "WASTEBIN_DATABASE_PATH";
const VAR_MAX_BODY_SIZE: &str = "WASTEBIN_MAX_BODY_SIZE";
const VAR_SIGNING_KEY: &str = "WASTEBIN_SIGNING_KEY";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("axum http error: {0}")]
    Axum(#[from] axum::http::Error),
    #[error("not allowed to delete")]
    Delete,
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("migrations error: {0}")]
    Migration(#[from] rusqlite_migration::Error),
    #[error("wrong size")]
    WrongSize,
    #[error("illegal characters")]
    IllegalCharacters,
    #[error("integer conversion error: {0}")]
    IntConversion(#[from] TryFromIntError),
    #[error("join error: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("syntax highlighting error: {0}")]
    SyntaxHighlighting(#[from] syntect::Error),
    #[error("syntax parsing error: {0}")]
    SyntaxParsing(#[from] syntect::parsing::ParsingError),
    #[error("time formatting error: {0}")]
    TimeFormatting(#[from] time::error::Format),
    #[error("could not parse cookie: {0}")]
    CookieParsing(String),
}

#[derive(thiserror::Error, Debug)]
enum EnvError {
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

impl From<Error> for StatusCode {
    fn from(err: Error) -> Self {
        match err {
            Error::Sqlite(err) => match err {
                rusqlite::Error::QueryReturnedNoRows => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
            Error::IllegalCharacters | Error::WrongSize | Error::CookieParsing(_) => {
                StatusCode::BAD_REQUEST
            }
            Error::Join(_)
            | Error::IntConversion(_)
            | Error::TimeFormatting(_)
            | Error::Migration(_)
            | Error::SyntaxHighlighting(_)
            | Error::SyntaxParsing(_)
            | Error::Axum(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Delete => StatusCode::FORBIDDEN,
        }
    }
}

pub(crate) fn make_app(max_body_size: usize) -> Router<cache::Layer> {
    Router::new()
        .merge(handler::routes())
        .layer(TimeoutLayer::new(Duration::from_secs(5)))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(DefaultBodyLimit::disable())
        .layer(DefaultBodyLimit::max(max_body_size))
}

async fn start() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let database = match env::var(VAR_DATABASE_PATH) {
        Ok(path) => Ok(Database::new(db::Open::Path(PathBuf::from(path)))?),
        Err(VarError::NotUnicode(_)) => Err(EnvError::DatabasePath),
        Err(VarError::NotPresent) => Ok(Database::new(db::Open::Memory)?),
    }?;

    let key = env::var(VAR_SIGNING_KEY).map_or_else(
        |_| Ok(Key::generate()),
        |s| Key::try_from(s.as_bytes()).map_err(|err| EnvError::SigningKey(err.to_string())),
    )?;

    let cache_size = env::var(VAR_CACHE_SIZE)
        .map_or_else(
            |_| Ok(NonZeroUsize::new(128).unwrap()),
            |s| s.parse::<NonZeroUsize>(),
        )
        .map_err(EnvError::CacheSize)?;

    let cache_layer = cache::Layer::new(database, cache_size, key);

    let addr: SocketAddr = env::var(VAR_ADDRESS_PORT)
        .unwrap_or_else(|_| "0.0.0.0:8088".to_string())
        .parse()
        .map_err(|_| EnvError::AddressPort)?;

    let max_body_size = env::var(VAR_MAX_BODY_SIZE)
        .map_or_else(|_| Ok(1024 * 1024), |s| s.parse::<usize>())
        .map_err(EnvError::MaxBodySize)?;

    tracing::debug!("serving on {addr}");
    tracing::debug!("caching {cache_size} paste highlights");
    tracing::debug!("restricting maximum body size to {max_body_size} bytes");

    let service: Router<()> = make_app(max_body_size).with_state(cache_layer.clone());

    let server = Server::bind(&addr)
        .serve(service.into_make_service())
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to listen to ctrl-c");
        });

    tokio::select! {
        res = server => {
            res?;
        },
        res = cache::purge_periodically(cache_layer) => {
            res?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    match start().await {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::SUCCESS
        }
    }
}
