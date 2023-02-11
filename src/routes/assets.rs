use crate::highlight;
use crate::{AppState, Router};
use axum::response::{IntoResponse, IntoResponseParts};
use axum::routing::get;
use axum::{headers, TypedHeader};
use bytes::Bytes;
use std::time::Duration;

fn css_headers() -> impl IntoResponseParts {
    (
        TypedHeader(headers::ContentType::from(mime::TEXT_CSS)),
        TypedHeader(headers::CacheControl::new().with_max_age(Duration::from_secs(3600))),
    )
}

fn style_css() -> impl IntoResponse {
    (css_headers(), highlight::DATA.style_css())
}

fn dark_css() -> impl IntoResponse {
    (css_headers(), highlight::DATA.dark_css())
}

fn light_css() -> impl IntoResponse {
    (css_headers(), highlight::DATA.light_css())
}

fn favicon() -> impl IntoResponse {
    (
        TypedHeader(headers::ContentType::png()),
        TypedHeader(headers::CacheControl::new().with_max_age(Duration::from_secs(86400))),
        Bytes::from_static(include_bytes!("../../assets/favicon.png")),
    )
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/favicon.png", get(|| async { favicon() }))
        .route("/style.css", get(|| async { style_css() }))
        .route("/dark.css", get(|| async { dark_css() }))
        .route("/light.css", get(|| async { light_css() }))
}
