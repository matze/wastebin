use crate::cache::Cache;
use crate::db::Database;
use crate::errors::Error;
use crate::handlers::extract::Theme;
use crate::handlers::{delete, download, html, insert, raw, theme};
use axum::extract::{DefaultBodyLimit, FromRef, Request, State};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::middleware::{Next, from_fn, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::{Router, get, post};
use axum_extra::extract::cookie::Key;
use http::header::{
    CONTENT_SECURITY_POLICY, REFERRER_POLICY, SERVER, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
    X_XSS_PROTECTION,
};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

mod assets;
mod cache;
mod crypto;
mod db;
mod env;
mod errors;
mod expiration;
mod handlers;
mod highlight;
mod id;
mod page;
#[cfg(test)]
mod test_helpers;

/// Reference counted [`page::Page`] wrapper.
pub(crate) type Page = Arc<page::Page>;

/// Reference counted [`highlight::Highlighter`] wrapper.
pub(crate) type Highlighter = Arc<highlight::Highlighter>;

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
    const SECURITY_HEADERS: [(HeaderName, HeaderValue); 7] = [
        (SERVER, HeaderValue::from_static(env!("CARGO_PKG_NAME"))),
        (
            CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'none'; script-src 'self'; img-src 'self' data: ; style-src 'self' data: ; font-src 'self' data: ; object-src 'none' ; base-uri 'none' ; frame-ancestors 'none' ; form-action 'self' ;",
            ),
        ),
        (REFERRER_POLICY, HeaderValue::from_static("same-origin")),
        (X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
        (X_FRAME_OPTIONS, HeaderValue::from_static("SAMEORIGIN")),
        (
            HeaderName::from_static("x-permitted-cross-domain-policies"),
            HeaderValue::from_static("none"),
        ),
        (X_XSS_PROTECTION, HeaderValue::from_static("1; mode=block")),
    ];

    (SECURITY_HEADERS, next.run(req).await)
}

async fn handle_service_errors(
    State(page): State<Page>,
    theme: Option<Theme>,
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
                description: String::from("payload exceeded limit"),
            },
        )
            .into_response(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE => (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            html::Error {
                page,
                theme,
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

async fn favicon(State(page): State<Page>) -> impl IntoResponse {
    page.assets.favicon.clone()
}

async fn style_css(State(page): State<Page>) -> impl IntoResponse {
    page.assets.css.style.clone()
}

async fn dark_css(State(page): State<Page>) -> impl IntoResponse {
    page.assets.css.dark.clone()
}

async fn light_css(State(page): State<Page>) -> impl IntoResponse {
    page.assets.css.light.clone()
}

async fn base_js(State(page): State<Page>) -> impl IntoResponse {
    page.assets.base_js.clone()
}

async fn index_js(State(page): State<Page>) -> impl IntoResponse {
    page.assets.index_js.clone()
}

async fn paste_js(State(page): State<Page>) -> impl IntoResponse {
    page.assets.paste_js.clone()
}

async fn serve(
    listener: TcpListener,
    state: AppState,
    timeout: Duration,
    max_body_size: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route(state.page.assets.favicon.route(), get(favicon))
        .route(state.page.assets.css.style.route(), get(style_css))
        .route(state.page.assets.css.dark.route(), get(dark_css))
        .route(state.page.assets.css.light.route(), get(light_css))
        .route(state.page.assets.base_js.route(), get(base_js))
        .route(state.page.assets.index_js.route(), get(index_js))
        .route(state.page.assets.paste_js.route(), get(paste_js))
        .route("/", get(html::index::get).post(insert::form::post))
        .route("/qr/:id", get(html::qr::get))
        .route("/burn/:id", get(html::burn::get))
        .route("/theme", get(theme::get))
        .route(
            "/:id",
            get(html::paste::get)
                .post(html::paste::get)
                .delete(delete::get),
        )
        .route("/dl/:id", get(download::get))
        .route("/raw/:id", get(raw::get))
        .route("/delete/:id", get(delete::get))
        .route("/api", post(insert::api::post))
        .layer(
            ServiceBuilder::new()
                .layer(DefaultBodyLimit::max(max_body_size))
                .layer(CompressionLayer::new())
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::new(timeout))
                .layer(from_fn_with_state(state.clone(), handle_service_errors))
                .layer(from_fn(security_headers_layer)),
            // .layer(from_fn(
        )
        .with_state(state);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
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
    let expirations = env::expiration_set()?;
    let theme = env::theme()?;
    let title = env::title();

    let cache = Cache::new(cache_size);
    let db = Database::new(method)?;

    tracing::debug!("serving on {addr}");
    tracing::debug!("caching {cache_size} paste highlights");
    tracing::debug!("restricting maximum body size to {max_body_size} bytes");
    tracing::debug!("enforcing a http timeout of {timeout:#?}");

    let page = Arc::new(page::Page::new(title, base_url, theme, expirations));
    let highlighter = Arc::new(highlight::Highlighter::default());
    let state = AppState {
        db,
        cache,
        key,
        page,
        highlighter,
    };

    let listener = TcpListener::bind(&addr).await?;
    serve(listener, state, timeout, max_body_size).await?;

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
