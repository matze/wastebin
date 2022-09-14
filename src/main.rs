use crate::db::Database;
use axum::http::StatusCode;
use axum::{Extension, Server};
use serde::{Deserialize, Serialize};
use std::env::{self, VarError};
use std::io;
use std::num::{NonZeroUsize, TryFromIntError};
use std::path::PathBuf;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

mod cache;
mod db;
mod handler;
mod highlight;
mod id;
#[cfg(test)]
mod test_helpers;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("axum http error: {0}")]
    Axum(#[from] axum::http::Error),
    #[error("deletion time expired")]
    DeletionTimeExpired,
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
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Entry {
    /// Content
    pub text: String,
    /// File extension
    pub extension: Option<String>,
    /// Expiration in seconds from now
    pub expires: Option<u32>,
    /// Delete if read
    pub burn_after_reading: Option<bool>,
    /// Seconds since creation
    pub seconds_since_creation: u32,
}

pub type Router = axum::Router<http_body::Limited<axum::body::Body>>;

impl From<Error> for StatusCode {
    fn from(err: Error) -> Self {
        match err {
            Error::Sqlite(err) => match err {
                rusqlite::Error::QueryReturnedNoRows => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
            Error::IllegalCharacters | Error::WrongSize | Error::DeletionTimeExpired => {
                StatusCode::BAD_REQUEST
            }
            Error::Join(_)
            | Error::IntConversion(_)
            | Error::TimeFormatting(_)
            | Error::Migration(_)
            | Error::SyntaxHighlighting(_)
            | Error::SyntaxParsing(_)
            | Error::Axum(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

pub(crate) fn make_app(cache_layer: cache::Layer, max_body_size: usize) -> axum::Router {
    Router::new()
        .merge(handler::routes())
        .layer(Extension(cache_layer))
        .layer(TimeoutLayer::new(Duration::from_secs(5)))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(RequestBodyLimitLayer::new(max_body_size))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let database = match env::var("WASTEBIN_DATABASE_PATH") {
        Ok(path) => Ok(Database::new(db::Open::Path(PathBuf::from(path)))?),
        Err(VarError::NotUnicode(_)) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "WASTEBIN_DATABASE_PATH contains non-unicode data",
        )),
        Err(VarError::NotPresent) => Ok(Database::new(db::Open::Memory)?),
    }?;

    let cache_size = env::var("WASTEBIN_CACHE_SIZE").map_or_else(
        |_| Ok(NonZeroUsize::new(128).unwrap()),
        |s| s.parse::<NonZeroUsize>(),
    )?;

    let cache_layer = cache::Layer::new(database, cache_size);

    let addr_port =
        env::var("WASTEBIN_ADDRESS_PORT").unwrap_or_else(|_| "0.0.0.0:8088".to_string());

    let max_body_size = env::var("WASTEBIN_MAX_BODY_SIZE")
        .map_or_else(|_| Ok(1024 * 1024), |s| s.parse::<usize>())?;

    tracing::debug!("serving on {addr_port}");
    tracing::debug!("caching {cache_size} paste highlights");
    tracing::debug!("restricting maximum body size to {max_body_size} bytes");

    let service = make_app(cache_layer.clone(), max_body_size).into_make_service();

    let server = Server::bind(&addr_port.parse()?)
        .serve(service)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to listen to ctrl-c");
        });

    tokio::select! {
        res = server => {
            res?
        },
        res = cache::purge_periodically(cache_layer) => {
            res?
        }
    }

    Ok(())
}
