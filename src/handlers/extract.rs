use crate::crypto;
use axum::extract::{Form, FromRequest, FromRequestParts, Query, Request};
use axum::http::request::Parts;
use serde::Deserialize;

/// Theme preference extracted from query string and cookie.
#[derive(Debug, Deserialize, Clone)]
pub enum Theme {
    #[serde(rename = "dark")]
    Dark,
    #[serde(rename = "light")]
    Light,
}

pub struct Password(pub crypto::Password);

/// Password header to encrypt a paste.
pub const PASSWORD_HEADER_NAME: http::HeaderName =
    http::HeaderName::from_static("wastebin-password");

#[axum::async_trait]
impl<S> FromRequestParts<S> for Theme
where
    S: Send + Sync,
{
    // Not extracting the preference is not an issue.
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        #[derive(Debug, Deserialize)]
        struct Data {
            pref: Theme,
        }

        let query: Option<Theme> = Query::from_request_parts(parts, state)
            .await
            .ok()
            .map(|Query(Data { pref })| pref);

        query.ok_or(())
    }
}

#[axum::async_trait]
impl<S> FromRequest<S> for Password
where
    S: Send + Sync,
{
    type Rejection = ();

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        #[derive(Deserialize, Debug)]
        struct Data {
            password: String,
        }

        if let Some(password) = req
            .headers()
            .get(PASSWORD_HEADER_NAME)
            .and_then(|header| header.to_str().ok())
        {
            return Ok(Password(password.as_bytes().to_vec().into()));
        }

        if let Some(data) = Option::<Form<Data>>::from_request(req, state)
            .await
            .ok()
            .flatten()
        {
            return Ok(Password(data.password.as_bytes().to_vec().into()));
        }

        Err(())
    }
}
