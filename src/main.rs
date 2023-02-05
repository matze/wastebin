#![forbid(unsafe_code)]

use crate::db::Database;
use crate::errors::Error;
use axum::extract::{DefaultBodyLimit, FromRef};
use axum::{Router, Server};
use axum_extra::extract::cookie::Key;
use std::process::ExitCode;
use std::time::Duration;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

mod db;
mod env;
mod errors;
mod handler;
mod highlight;
mod id;
mod pages;
#[cfg(test)]
mod test_helpers;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub key: Key,
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.key.clone()
    }
}

pub(crate) fn make_app(max_body_size: usize) -> Router<AppState> {
    Router::new().merge(handler::routes()).layer(
        ServiceBuilder::new()
            .layer(DefaultBodyLimit::max(max_body_size))
            .layer(DefaultBodyLimit::disable())
            .layer(CompressionLayer::new())
            .layer(TraceLayer::new_for_http())
            .layer(TimeoutLayer::new(Duration::from_secs(5))),
    )
}

async fn start() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let cache_size = env::cache_size()?;
    let method = env::database_method()?;
    let key = env::signing_key()?;
    let addr = env::addr()?;
    let max_body_size = env::max_body_size()?;
    let cache = db::Cache::new(cache_size);
    let db = Database::new(method, cache)?;
    let state = AppState { db, key };

    tracing::debug!("serving on {addr}");
    tracing::debug!("caching {cache_size} paste highlights");
    tracing::debug!("restricting maximum body size to {max_body_size} bytes");

    let service: Router<()> = make_app(max_body_size).with_state(state);

    Server::bind(&addr)
        .serve(service.into_make_service())
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to listen to ctrl-c");
        })
        .await?;

    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    match start().await {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::FAILURE
        }
    }
}
