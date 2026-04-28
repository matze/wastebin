use std::convert::Infallible;

use axum::extract::{
    Form, FromRef, FromRequest, FromRequestParts, OptionalFromRequest, OptionalFromRequestParts,
    Request,
};
use axum::http::request::Parts;
use axum::response::Redirect;
use axum_extra::extract::cookie::Key;
use axum_extra::extract::{CookieJar, SignedCookieJar};
use serde::Deserialize;

use wastebin_core::crypto;

use crate::i18n::Lang;

/// A safe redirect back to the referer.
///
/// Extracts the `Referer` header and strips it down to just the path (and query string),
/// preventing open redirects via external referer values. Falls back to `"/"`.
pub(crate) struct SafeReferer(pub Redirect);

/// Theme extractor, extracted from the `pref` cookie.
#[derive(Debug, Deserialize, Clone)]
pub(crate) enum Theme {
    #[serde(rename = "dark")]
    Dark,
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "system")]
    System,
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
            Theme::System => f.write_str("system"),
        }
    }
}

impl std::str::FromStr for Theme {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "dark" => Ok(Theme::Dark),
            "light" => Ok(Theme::Light),
            "system" => Ok(Theme::System),
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

impl<S> FromRequestParts<S> for SafeReferer
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let redirect = parts
            .headers
            .get(http::header::REFERER)
            .and_then(|referer| referer.to_str().ok())
            .map(|referer| {
                if referer.starts_with('/') && !referer.starts_with("//") {
                    Redirect::to(referer)
                } else {
                    referer
                        .parse::<url::Url>()
                        .ok()
                        .map(|url| {
                            let path = url.path();
                            url.query().map_or_else(
                                || Redirect::to(path),
                                |q| Redirect::to(&format!("{path}?{q}")),
                            )
                        })
                        .unwrap_or_else(|| Redirect::to("/"))
                }
            })
            .unwrap_or_else(|| Redirect::to("/"));

        Ok(SafeReferer(redirect))
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

/// Map a single language tag (e.g. `en`, `de-AT`) to a supported [`Lang`].
fn lang_from_tag(tag: &str) -> Option<Lang> {
    match tag.split('-').next()?.trim() {
        "en" | "eN" | "En" | "EN" => Some(Lang::En),
        "de" | "dE" | "De" | "DE" => Some(Lang::De),
        _ => None,
    }
}

/// Pick the best supported language from an `Accept-Language` header value,
/// honoring `q=` weights. Falls back to the default language if nothing
/// matches.
fn lang_from_accept_language(header: &str) -> Lang {
    header
        .split(',')
        .enumerate()
        .filter_map(|(idx, entry)| {
            let mut parts = entry.split(';');
            let tag = parts.next().map(str::trim).filter(|t| !t.is_empty())?;
            let lang = lang_from_tag(tag)?;

            let q = parts
                .find_map(|p| {
                    let p = p.trim();
                    p.strip_prefix("q=").or_else(|| p.strip_prefix("Q="))
                })
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(1.0);

            // Use position as a tie-breaker so the first listed entry wins
            // when weights are equal.
            #[expect(clippy::cast_precision_loss)]
            let weighted = q - (idx as f32) * 1e-6;
            Some((weighted, lang))
        })
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map_or(Lang::default(), |(_, l)| l)
}

impl<S> FromRequestParts<S> for Lang
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts
            .headers
            .get(http::header::ACCEPT_LANGUAGE)
            .and_then(|v| v.to_str().ok())
            .map_or_else(Lang::default, lang_from_accept_language))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_highest_q() {
        assert_eq!(lang_from_accept_language("en;q=0.5,de;q=0.9"), Lang::De);
    }

    #[test]
    fn defaults_to_english_when_unsupported() {
        assert_eq!(lang_from_accept_language("ja,fr;q=0.7"), Lang::En);
    }

    #[test]
    fn handles_region_subtags() {
        assert_eq!(lang_from_accept_language("de-AT"), Lang::De);
    }

    #[test]
    fn first_listed_wins_on_tie() {
        // Both implicit q=1.0; first listed should win.
        assert_eq!(lang_from_accept_language("de,en"), Lang::De);
        assert_eq!(lang_from_accept_language("en,de"), Lang::En);
    }
}
