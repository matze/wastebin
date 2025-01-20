use crate::highlight::{self, DATA};
use crate::{AppState, Router};
use axum::response::{IntoResponse, IntoResponseParts};
use axum::routing::get;
use axum_extra::{headers, TypedHeader};
use bytes::Bytes;
use std::time::Duration;

/// Asset maximum age of six months.
const MAX_AGE: Duration = Duration::from_secs(60 * 60 * 24 * 30 * 6);

fn css_from(content: &'static str) -> impl IntoResponse {
    (
        (
            TypedHeader(headers::ContentType::from(mime::TEXT_CSS)),
            TypedHeader(headers::CacheControl::new().with_max_age(MAX_AGE)),
        ),
        content,
    )
}

fn js_from(content: &'static str) -> impl IntoResponse {
    (
        (
            TypedHeader(headers::ContentType::from(mime::TEXT_JAVASCRIPT)),
            TypedHeader(headers::CacheControl::new().with_max_age(MAX_AGE)),
        ),
        content,
    )
}

pub fn routes() -> Router<AppState> {
    let style_url = format!("/{}", &DATA.style.name);
    let index_url = format!("/{}", &DATA.index.name);
    let paste_url = format!("/{}", &DATA.paste.name);

    Router::new()
        .route(
            "/favicon.ico",
            get(|| async {
                (
                    TypedHeader(headers::ContentType::png()),
                    TypedHeader(headers::CacheControl::new().with_max_age(MAX_AGE)),
                    Bytes::from_static(include_bytes!("../../assets/favicon.png")),
                )
            }),
        )
        .route(&style_url, get(|| async { css_from(DATA.style.content) }))
        .route(
            "/dark.css",
            get(|| async { css_from(highlight::DARK_CSS.as_str()) }),
        )
        .route(
            "/light.css",
            get(|| async { css_from(highlight::LIGHT_CSS.as_str()) }),
        )
        .route(&index_url, get(|| async { js_from(&DATA.index.content) }))
        .route(&paste_url, get(|| async { js_from(&DATA.paste.content) }))
}
