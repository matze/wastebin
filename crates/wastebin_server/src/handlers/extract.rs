use axum::extract::{Form, FromRef, FromRequest, FromRequestParts, Request};
use axum::http::request::Parts;
use axum_extra::extract::cookie::Key;
use axum_extra::extract::{CookieJar, SignedCookieJar};
use serde::Deserialize;
use wastebin_core::crypto;

/// Theme extractor, extracted from the `pref` cookie.
#[derive(Debug, Deserialize, Clone)]
pub(crate) enum Theme {
    #[serde(rename = "dark")]
    Dark,
    #[serde(rename = "light")]
    Light,
}

/// Theme preference for use in shared [`axum::extract::Query`]'s.
#[derive(Debug, Deserialize)]
pub(crate) struct Preference {
    pub pref: Theme,
}

/// Password extractor.
pub(crate) struct Password(pub crypto::Password);

/// Uid cookie value extractor, extracted from the `uid` cookie.
pub(crate) struct Uid(pub i64);

/// Password header to encrypt a paste.
pub(crate) const PASSWORD_HEADER_NAME: http::HeaderName =
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
impl<S> FromRequestParts<S> for Uid
where
    S: Send + Sync,
    Key: FromRef<S>,
{
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let jar: SignedCookieJar<crate::Key> = SignedCookieJar::from_request_parts(parts, state)
            .await
            .map_err(|_| ())?;

        let uid = jar
            .get("uid")
            .map(|cookie| cookie.value_trimmed().parse::<i64>())
            .transpose()
            .map_err(|_| ())?
            .map(Uid)
            .ok_or(())?;

        Ok(uid)
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
