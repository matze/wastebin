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

#[cfg(test)]
mod tests {
    use crate::test_helpers::{Client, StoreCookies};

    #[tokio::test]
    async fn redirect_with_cookie() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(true)).await;

        let response = client
            .get("/theme")
            .query(&[("pref", "dark")])
            .send()
            .await?;

        assert!(response.status().is_redirection());

        let cookie = response
            .cookies()
            .find(|cookie| cookie.name() == "pref")
            .unwrap();

        assert_eq!(cookie.value(), "dark");

        Ok(())
    }
}
