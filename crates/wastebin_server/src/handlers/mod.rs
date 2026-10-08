pub mod delete;
pub mod download;
pub mod extract;
pub mod html;
pub mod insert;
pub mod raw;
pub mod robots;
pub mod theme;

use axum::http::{HeaderValue, header};
use axum::response::Response;
use axum_extra::extract::cookie::{Cookie, SameSite};

use crate::handlers::extract::serialize_uids;

/// Marks a response as uncacheable by browsers and intermediaries.
///
/// Used for password-protected pastes so that decrypted content is never stored.
pub(crate) trait NoStore {
    /// Set `Cache-Control: no-store`.
    #[must_use]
    fn no_store(self) -> Self;
}

impl NoStore for Response {
    fn no_store(mut self) -> Self {
        self.headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));

        self
    }
}

/// Build a cookie with `HttpOnly`, `SameSite=Strict` and `Path=/`.
///
/// This is for cookies such as the theme preference. The `Secure` attribute is left to
/// [`uid_cookie`].
pub(crate) fn cookie(name: &str, value: String) -> Cookie<'static> {
    let mut cookie = Cookie::new(name.to_owned(), value);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Strict);
    cookie.set_path("/");
    cookie
}

/// Build the `uid` cookie which authorizes deletion of pastes.
pub(crate) fn uid_cookie(uids: &[i64]) -> Cookie<'static> {
    let mut cookie = cookie("uid", serialize_uids(uids));
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(true);
    cookie
}
