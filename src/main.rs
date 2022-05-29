use crate::db::Database;
use axum::Server;
use std::path::PathBuf;
use tower_http::trace::TraceLayer;

mod db;
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
    Syntax(#[from] syntect::Error),
    #[error("time formatting error: {0}")]
    TimeFormatting(#[from] time::error::Format),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let path = PathBuf::from("./foo.db");
    let database = Database::new(db::Open::Path(path))?;
    let router = srv::new_router(database.clone()).layer(TraceLayer::new_for_http());
    let server = Server::bind(&"0.0.0.0:8888".parse()?)
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
