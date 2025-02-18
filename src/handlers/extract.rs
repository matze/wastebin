use crate::crypto;
use axum::extract::{Form, FromRequest, FromRequestParts, Request};
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;
use serde::Deserialize;

/// Theme extracted from the `pref` cookie.
#[derive(Debug, Deserialize, Clone)]
pub enum Theme {
    #[serde(rename = "dark")]
    Dark,
    #[serde(rename = "light")]
    Light,
}

/// Theme preference for use in shared [`axum::extract::Query`]'s.
#[derive(Debug, Deserialize)]
pub struct Preference {
    pub pref: Theme,
}

pub struct Password(pub crypto::Password);

/// Password header to encrypt a paste.
pub const PASSWORD_HEADER_NAME: http::HeaderName =
    http::HeaderName::from_static("wastebin-password");

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Theme::Dark => f.write_str("dark"),
            Theme::Light => f.write_str("light"),
        }
    }
}

impl std::str::FromStr for Theme {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "dark" => Ok(Theme::Dark),
            "light" => Ok(Theme::Light),
            _ => Err(()),
        }
    }
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for Theme
where
    S: Send + Sync,
{
    // Not extracting the preference is not an issue.
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_request_parts(parts, state)
            .await
            .map_err(|_| ())?;

        jar.get("pref")
            .map(|cookie| cookie.value_trimmed().parse())
            .transpose()?
            .ok_or(())
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
