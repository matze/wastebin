use crate::db::Database;
use axum::http::StatusCode;
use axum::Server;
use axum::{Extension, Router};
use serde::{Deserialize, Serialize};
use std::env::{self, VarError};
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

mod db;
mod highlight;
mod id;
mod rest;
mod web;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("migrations error: {0}")]
    Migration(#[from] rusqlite_migration::Error),
    #[error("wrong size")]
    WrongSize,
    #[error("illegal characters")]
    IllegalCharacters,
    #[error("join error: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("syntax highlighting error: {0}")]
    SyntaxHighlighting(#[from] syntect::Error),
    #[error("syntax parsing error: {0}")]
    SyntaxParsing(#[from] syntect::parsing::ParsingError),
    #[error("time formatting error: {0}")]
    TimeFormatting(#[from] time::error::Format),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    /// Content
    pub text: String,
    /// File extension
    pub extension: Option<String>,
    /// Expiration in seconds from now
    pub expires: Option<u32>,
    /// Delete if read
    pub burn_after_reading: Option<bool>,
}

pub type Cache = Arc<Mutex<lru::LruCache<String, String>>>;

impl From<Error> for StatusCode {
    fn from(err: Error) -> Self {
        match err {
            Error::Sqlite(err) => match err {
                rusqlite::Error::QueryReturnedNoRows => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
            Error::Migration(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::TimeFormatting(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::IllegalCharacters => StatusCode::BAD_REQUEST,
            Error::WrongSize => StatusCode::BAD_REQUEST,
            Error::Join(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::SyntaxHighlighting(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::SyntaxParsing(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
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

    let addr_port =
        env::var("WASTEBIN_ADDRESS_PORT").unwrap_or_else(|_| "0.0.0.0:8088".to_string());

    let cache_size =
        env::var("WASTEBIN_CACHE_SIZE").map_or_else(|_| Ok(128), |s| s.parse::<usize>())?;

    tracing::debug!("Caching {cache_size} paste highlights");

    let cache: Cache = Arc::new(Mutex::new(lru::LruCache::new(cache_size)));

    let service = Router::new()
        .merge(web::routes())
        .merge(rest::routes())
        .layer(Extension(database.clone()))
        .layer(Extension(cache))
        .layer(TimeoutLayer::new(Duration::from_secs(5)))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .into_make_service();

    let server = Server::bind(&addr_port.parse()?)
        .serve(service)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to listen to ctrl-c");
        });

    tokio::select! {
        _ = server => {},
        _ = db::purge_periodically(database) => {}
    }

    Ok(())
}
