mod assets;
mod cache;
mod env;
mod errors;
mod handlers;
mod i18n;
mod page;
#[cfg(test)]
mod test_helpers;

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{DefaultBodyLimit, FromRef, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::middleware::{Next, from_fn, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, Router, get, post};
use axum_extra::extract::cookie::Key;
use http::header::{
    CONTENT_SECURITY_POLICY, REFERRER_POLICY, SERVER, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
    X_XSS_PROTECTION,
};
use tokio::net::{TcpListener, UnixListener};
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::compression::predicate::{DefaultPredicate, Predicate, SizeAbove};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::assets::Asset;
use crate::cache::Cache;
use crate::errors::Error;
use crate::handlers::extract::Theme;
use crate::handlers::{delete, download, html, insert, raw, robots, theme};
use crate::i18n::Lang;
use wastebin_core::db::Database;

/// Reference counted [`page::Page`] wrapper.
pub(crate) type Page = Arc<page::Page>;

/// Reference counted [`highlight::Highlighter`] wrapper.
pub(crate) type Highlighter = Arc<wastebin_highlight::Highlighter>;

#[derive(Clone)]
pub(crate) struct AppState {
    db: Database,
    cache: Cache,
    key: Key,
    page: Page,
    highlighter: Highlighter,
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.key.clone()
    }
}

impl FromRef<AppState> for Highlighter {
    fn from_ref(state: &AppState) -> Self {
        state.highlighter.clone()
    }
}

impl FromRef<AppState> for Page {
    fn from_ref(state: &AppState) -> Self {
        state.page.clone()
    }
}

impl FromRef<AppState> for Database {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}

impl FromRef<AppState> for Cache {
    fn from_ref(state: &AppState) -> Self {
        state.cache.clone()
    }
}

async fn security_headers_layer(req: Request, next: Next) -> impl IntoResponse {
    // Rendered Markdown may embed remote images via `![](…)`; relax img-src for that route only.
    const CSP_STRICT: HeaderValue = HeaderValue::from_static(
        "default-src 'none'; script-src 'self'; img-src 'self' data: ; style-src 'self' data: ; font-src 'self' data: ; object-src 'none' ; base-uri 'none' ; frame-ancestors 'none' ; form-action 'self' ;",
    );
    const CSP_RENDERED: HeaderValue = HeaderValue::from_static(
        "default-src 'none'; script-src 'self'; img-src * data: ; style-src 'self' data: ; font-src 'self' data: ; object-src 'none' ; base-uri 'none' ; frame-ancestors 'none' ; form-action 'self' ;",
    );

    let csp = if req.uri().path().starts_with("/md/") {
        CSP_RENDERED
    } else {
        CSP_STRICT
    };

    let headers: [(HeaderName, HeaderValue); 7] = [
        (SERVER, HeaderValue::from_static(env!("CARGO_PKG_NAME"))),
        (CONTENT_SECURITY_POLICY, csp),
        (REFERRER_POLICY, HeaderValue::from_static("same-origin")),
        (X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
        (X_FRAME_OPTIONS, HeaderValue::from_static("SAMEORIGIN")),
        (
            HeaderName::from_static("x-permitted-cross-domain-policies"),
            HeaderValue::from_static("none"),
        ),
        (X_XSS_PROTECTION, HeaderValue::from_static("1; mode=block")),
    ];

    (headers, next.run(req).await)
}

async fn handle_service_errors(
    State(page): State<Page>,
    theme: Option<Theme>,
    lang: Lang,
    req: Request,
    next: Next,
) -> Response {
    let response = next.run(req).await;

    match response.status() {
        StatusCode::PAYLOAD_TOO_LARGE => (
            StatusCode::PAYLOAD_TOO_LARGE,
            html::Error {
                page,
                theme,
                lang,
                description: String::from("payload exceeded limit"),
            },
        )
            .into_response(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE => (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            html::Error {
                page,
                theme,
                lang,
                description: String::from("unsupported media type"),
            },
        )
            .into_response(),
        _ => response,
    }
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

/// Route serving `asset` in the content coding accepted by the request.
fn asset_route(asset: &Asset) -> MethodRouter<AppState> {
    let asset = asset.clone();
    get(move |headers: HeaderMap| async move { asset.to_response(&headers) })
}

fn make_app(state: AppState, timeout: Duration, max_body_size: usize) -> Router {
    let assets = [
        &state.page.assets.favicon,
        &state.page.assets.css.style,
        &state.page.assets.css.dark,
        &state.page.assets.css.light,
        &state.page.assets.css.no_js,
        &state.page.assets.index_js,
        &state.page.assets.paste_js,
        &state.page.assets.burn_js,
        &state.page.assets.password_toggle_js,
    ];

    let mut app = Router::new();

    for asset in assets {
        app = app.route(asset.route(), asset_route(asset));
    }

    app.route("/", get(html::index::get).post(insert::api::post))
        .route("/robots.txt", get(robots::get))
        .route("/theme", get(theme::get))
        .route("/new", post(insert::form::post))
        .route("/qr/{id}", get(html::qr::get))
        .route(
            "/md/{id}",
            get(html::rendered::get).post(html::rendered::get),
        )
        .route("/burn/{id}", get(html::burn::get))
        .route(
            "/{id}",
            get(html::paste::get)
                .post(html::paste::get)
                .delete(delete::api::delete),
        )
        .route("/dl/{id}", get(download::get))
        .route("/raw/{id}", get(raw::get))
        .route("/delete/{id}", post(delete::form::delete))
        .layer(
            ServiceBuilder::new()
                .layer(DefaultBodyLimit::max(max_body_size))
                .layer(
                    CompressionLayer::new()
                        .compress_when(DefaultPredicate::new().and(SizeAbove::new(256))),
                )
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::REQUEST_TIMEOUT,
                    timeout,
                ))
                .layer(from_fn(security_headers_layer))
                .layer(from_fn_with_state(state.clone(), handle_service_errors)),
        )
        .with_state(state)
}

async fn start() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let cache_size = env::cache_size()?;
    let method = env::database_method()?;
    let key = env::signing_key()?;
    let socket_type = env::socket_type()?;
    let max_body_size = env::max_body_size()?;
    let base_url = env::base_url()?;
    let timeout = env::http_timeout()?;
    let expirations = env::expiration_set()?;
    let theme = env::theme()?;
    let title = env::title();

    let cache = Cache::new(cache_size)?;
    let (db, db_handler) = Database::new(method)?;

    tracing::debug!("serving on {socket_type}");
    tracing::debug!("caching {cache_size} paste highlights");
    tracing::debug!("restricting maximum body size to {max_body_size} bytes");
    tracing::debug!("enforcing a http timeout of {timeout:#?}");

    let page = Arc::new(page::Page::new(
        title,
        base_url,
        theme,
        expirations,
        max_body_size,
    ));
    let highlighter = Arc::new(wastebin_highlight::Highlighter::default());
    let state = AppState {
        db,
        cache,
        key,
        page,
        highlighter,
    };

    let app = make_app(state, timeout, max_body_size);

    let serve = async {
        match socket_type {
            env::SocketType::Tcp(addr) => {
                let listener = TcpListener::bind(addr).await?;
                axum::serve(listener, app)
                    .with_graceful_shutdown(shutdown_signal())
                    .await?;
            }
            env::SocketType::Unix(path) => {
                let listener = UnixListener::bind(path)?;
                axum::serve(listener, app)
                    .with_graceful_shutdown(shutdown_signal())
                    .await?;
            }
        }

        Ok::<(), Box<dyn std::error::Error>>(())
    };

    tokio::try_join!(serve, async { db_handler.await.map_err(Into::into) })?;

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
