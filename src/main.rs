use crate::cache::Cache;
use crate::db::Database;
use crate::env::BASE_PATH;
use crate::errors::Error;
use axum::extract::{DefaultBodyLimit, FromRef};
use axum::Router;
use axum_extra::extract::cookie::Key;
use std::num::NonZeroU32;
use std::process::ExitCode;
use std::time::Duration;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use url::Url;

mod cache;
mod crypto;
mod db;
mod env;
mod errors;
mod highlight;
mod id;
mod pages;
pub(crate) mod routes;
#[cfg(test)]
mod test_helpers;

#[derive(Clone)]
pub struct AppState {
    db: Database,
    cache: Cache,
    key: Key,
    base_url: Option<Url>,
    max_expiration: Option<NonZeroU32>,
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.key.clone()
    }
}

pub(crate) fn make_app(max_body_size: usize, timeout: Duration) -> Router<AppState> {
    Router::new()
        .nest(BASE_PATH.path(), routes::routes())
        .layer(
            ServiceBuilder::new()
                .layer(DefaultBodyLimit::max(max_body_size))
                .layer(DefaultBodyLimit::disable())
                .layer(CompressionLayer::new())
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::new(timeout)),
        )
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!("received signal, exiting ...");
}

async fn start() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let cache_size = env::cache_size()?;
    let method = env::database_method()?;
    let key = env::signing_key()?;
    let addr = env::addr()?;
    let max_body_size = env::max_body_size()?;
    let base_url = env::base_url()?;
    let timeout = env::http_timeout()?;
    let max_expiration = env::max_paste_expiration()?;

    let cache = Cache::new(cache_size);
    let db = Database::new(method)?;
    let state = AppState {
        db,
        cache,
        key,
        base_url,
        max_expiration,
    };

    tracing::debug!("serving on {addr}");
    tracing::debug!("caching {cache_size} paste highlights");
    tracing::debug!("restricting maximum body size to {max_body_size} bytes");
    tracing::debug!("enforcing a http timeout of {timeout:#?}");
    tracing::debug!("maximum expiration time of {max_expiration:?} seconds");

    let service = make_app(max_body_size, timeout).with_state(state);
    let listener = TcpListener::bind(&addr).await?;

    axum::serve(listener, service)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    match start().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::FAILURE
        }
    }
}
