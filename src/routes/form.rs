use std::num::NonZeroU32;

use crate::db::write;
use crate::env::BASE_PATH;
use crate::{pages, AppState, Error};
use axum::extract::{Form, State};
use axum::response::Redirect;
use axum_extra::extract::cookie::{Cookie, SameSite, SignedCookieJar};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub text: String,
    pub extension: Option<String>,
    pub expires: String,
    pub password: String,
}

impl From<Entry> for write::Entry {
    fn from(entry: Entry) -> Self {
        let burn_after_reading = Some(entry.expires == "burn");
        let password = (!entry.password.is_empty()).then_some(entry.password);

        let expires = match entry.expires.parse::<NonZeroU32>() {
            Err(_) => None,
            Ok(secs) => Some(secs),
        };

        Self {
            text: entry.text,
            extension: entry.extension,
            expires,
            burn_after_reading,
            uid: None,
            password,
        }
    }
}

pub async fn insert(
    state: State<AppState>,
    jar: SignedCookieJar,
    Form(entry): Form<Entry>,
    is_https: bool,
) -> Result<(SignedCookieJar, Redirect), pages::ErrorResponse<'static>> {
    // Retrieve uid from cookie or generate a new one.
    let uid = if let Some(cookie) = jar.get("uid") {
        cookie
            .value()
            .parse::<i64>()
            .map_err(|err| Error::CookieParsing(err.to_string()))?
    } else {
        state.db.next_uid().await?
    };

    let mut entry: write::Entry = entry.into();
    entry.uid = Some(uid);

    if let Some(max_exp) = state.max_expiration {
        entry.expires = entry
            .expires
            .map_or_else(|| Some(max_exp), |value| Some(value.min(max_exp)));
    }

    let burn = entry.burn_after_reading.unwrap_or(false);
    let extension = entry.extension.clone();

    let id = state.db.insert(entry).await?;

    let mut url = id.to_url_path(extension.as_deref());

    if burn {
        url = format!("burn/{url}");
    }

    let url_with_base = BASE_PATH.join(&url);

    let cookie = Cookie::build(("uid", uid.to_string()))
        .http_only(true)
        .secure(is_https)
        .same_site(SameSite::Strict)
        .build();

    let jar = jar.add(cookie);
    Ok((jar, Redirect::to(&url_with_base)))
}
