use axum::extract::Query;
use axum::response::IntoResponse;
use axum_extra::extract::CookieJar;

use crate::handlers::cookie;
use crate::handlers::extract::{Preference, SafeReferer};

/// GET handler to switch theme by setting the pref cookie and redirecting back to the referer.
pub async fn get(
    SafeReferer(redirect): SafeReferer,
    jar: CookieJar,
    Query(pref): Query<Preference>,
) -> impl IntoResponse {
    let cookie = cookie("pref", pref.pref.to_string());
    (jar.add(cookie), redirect)
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::{Client, StoreCookies};
    use http::header::REFERER;

    #[tokio::test]
    async fn external_referer_redirects_to_path_only() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(true)).await;

        let response = client
            .get("/theme")
            .header(REFERER, "https://evil.example.com/phish?bait=1")
            .query(&[("pref", "dark")])
            .send()
            .await?;

        assert!(response.status().is_redirection());
        let location = response.headers().get("location").unwrap().to_str()?;
        assert_eq!(location, "/phish?bait=1");

        Ok(())
    }

    #[tokio::test]
    async fn protocol_relative_referer_falls_back_to_root() -> Result<(), Box<dyn std::error::Error>>
    {
        let client = Client::new(StoreCookies(true)).await;

        let response = client
            .get("/theme")
            .header(REFERER, "//evil.example.com/phish")
            .query(&[("pref", "dark")])
            .send()
            .await?;

        assert!(response.status().is_redirection());
        let location = response.headers().get("location").unwrap().to_str()?;
        assert_eq!(location, "/");

        Ok(())
    }

    #[tokio::test]
    async fn redirect_with_cookie() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(true)).await;

        let response = client
            .get("/theme")
            .header(REFERER, "/foo")
            .query(&[("pref", "dark")])
            .send()
            .await?;

        assert!(response.status().is_redirection());

        let location = response.headers().get("location").unwrap().to_str()?;
        assert_eq!(location, "/foo");

        let cookie = response
            .cookies()
            .find(|cookie| cookie.name() == "pref")
            .unwrap();

        assert_eq!(cookie.value(), "dark");
        assert_eq!(cookie.path().unwrap(), "/");
        assert!(cookie.http_only());
        assert!(cookie.same_site_strict());

        Ok(())
    }

    #[tokio::test]
    async fn absolute_referer_with_scheme_relative_path_falls_back()
    -> Result<(), Box<dyn std::error::Error>> {
        // The URL parses, but its PATH is itself scheme-relative (`//host/...`).
        let client = Client::new(StoreCookies(true)).await;

        let response = client
            .get("/theme")
            .header(REFERER, "https://evil.example.com//attacker.example.com/x")
            .query(&[("pref", "dark")])
            .send()
            .await?;

        assert!(response.status().is_redirection());
        let location = response.headers().get("location").unwrap().to_str()?;
        assert_eq!(location, "/");

        Ok(())
    }

    #[tokio::test]
    async fn backslash_relative_referer_falls_back() -> Result<(), Box<dyn std::error::Error>> {
        // `/\host` starts with a slash but a browser treats it as `//host`.
        let client = Client::new(StoreCookies(true)).await;

        let response = client
            .get("/theme")
            .header(REFERER, "/\\evil.example.com")
            .query(&[("pref", "dark")])
            .send()
            .await?;

        assert!(response.status().is_redirection());
        let location = response.headers().get("location").unwrap().to_str()?;
        assert_eq!(location, "/");

        Ok(())
    }
}
