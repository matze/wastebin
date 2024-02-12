use crate::highlight::data;
use crate::{env, AppState, Router};
use axum::response::{IntoResponse, IntoResponseParts};
use axum::routing::get;
use axum_extra::{headers, TypedHeader};
use bytes::Bytes;

fn css_headers() -> impl IntoResponseParts {
    (
        TypedHeader(headers::ContentType::from(mime::TEXT_CSS)),
        TypedHeader(headers::CacheControl::new().with_max_age(env::CSS_MAX_AGE)),
    )
}

fn style_css() -> impl IntoResponse {
    (css_headers(), data().style.content.to_string())
}

fn dark_css() -> impl IntoResponse {
    (css_headers(), data().dark.content.to_string())
}

fn light_css() -> impl IntoResponse {
    (css_headers(), data().light.content.to_string())
}

fn favicon() -> impl IntoResponse {
    (
        TypedHeader(headers::ContentType::png()),
        TypedHeader(headers::CacheControl::new().with_max_age(env::FAVICON_MAX_AGE)),
        Bytes::from_static(include_bytes!("../../assets/favicon.png")),
    )
}

pub fn routes() -> Router<AppState> {
    let style_name = &data().style.name;
    Router::new()
        .route("/favicon.png", get(|| async { favicon() }))
        .route(&format!("/{style_name}"), get(|| async { style_css() }))
        .route("/dark.css", get(|| async { dark_css() }))
        .route("/light.css", get(|| async { light_css() }))
}
