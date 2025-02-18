use crate::handlers::extract::Preference;
use axum::extract::Query;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect};
use http::header::{REFERER, SET_COOKIE};

/// GET handler to switch theme by setting the pref cookie and redirecting back to the referer.
pub async fn get(headers: HeaderMap, Query(pref): Query<Preference>) -> impl IntoResponse {
    let response = headers
        .get(REFERER)
        .and_then(|referer| referer.to_str().ok())
        .map_or_else(|| Redirect::to("/"), Redirect::to);

    ([(SET_COOKIE, format!("pref={}", pref.pref))], response)
}
