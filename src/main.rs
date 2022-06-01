use crate::db::Database;
use axum::Server;
use std::env::{self, VarError};
use std::io;
use std::path::PathBuf;
use tower_http::trace::TraceLayer;

mod db;
mod highlight;
mod id;
mod srv;

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
    #[error("join error")]
    Join(#[from] tokio::task::JoinError),
    #[error("syntax highlighting error: {0}")]
    SyntaxHighlighting(#[from] syntect::Error),
    #[error("syntax parsing error: {0}")]
    SyntaxParsing(#[from] syntect::parsing::ParsingError),
    #[error("time formatting error: {0}")]
    TimeFormatting(#[from] time::error::Format),
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

    let router = srv::new_router(database.clone()).layer(TraceLayer::new_for_http());

    let server = Server::bind(&addr_port.parse()?)
        .serve(router.into_make_service())
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
