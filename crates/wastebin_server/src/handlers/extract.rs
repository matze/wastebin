use axum::extract::{
    Form, FromRef, FromRequest, FromRequestParts, OptionalFromRequest, OptionalFromRequestParts,
    Request,
};
use axum::http::request::Parts;
use axum_extra::extract::cookie::Key;
use axum_extra::extract::{CookieJar, SignedCookieJar};
use serde::Deserialize;
use std::convert::Infallible;
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

impl<S> OptionalFromRequestParts<S> for Theme
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        let jar = CookieJar::from_request_parts(parts, state).await;

        jar.map(|jar| {
            jar.get("pref")
                .and_then(|cookie| cookie.value_trimmed().parse::<Theme>().ok())
        })
    }
}

impl<S> FromRequestParts<S> for Uid
where
    S: Send + Sync,
    Key: FromRef<S>,
{
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let jar = SignedCookieJar::<crate::Key>::from_request_parts(parts, state)
            .await
            .map_err(|_| ())?;

        jar.get("uid")
            .map(|cookie| {
                cookie
                    .value_trimmed()
                    .parse::<i64>()
                    .map(Uid)
                    .map_err(|_| ())
            })
            .ok_or(())?
    }
}

impl<S> OptionalFromRequestParts<S> for Uid
where
    S: Send + Sync,
    Key: FromRef<S>,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        Ok(
            <Uid as FromRequestParts<S>>::from_request_parts(parts, state)
                .await
                .ok(),
        )
    }
}

impl<S> OptionalFromRequest<S> for Password
where
    S: Send + Sync,
{
    type Rejection = ();

    async fn from_request(req: Request, state: &S) -> Result<Option<Self>, Self::Rejection> {
        #[derive(Deserialize, Debug)]
        struct Data {
            password: String,
        }

        let password = req
            .headers()
            .get(PASSWORD_HEADER_NAME)
            .and_then(|header| header.to_str().ok())
            .map(|value| Password(value.as_bytes().to_vec().into()));

        if password.is_some() {
            return Ok(password);
        }

        Ok(Form::<Data>::from_request(req, state)
            .await
            .ok()
            .map(|data| Password(data.password.as_bytes().to_vec().into())))
    }
}
