use std::num::NonZeroU32;

use axum::extract::{Form, State};
use axum::response::{IntoResponse, Redirect};
use axum_extra::extract::cookie::SignedCookieJar;
use serde::{Deserialize, Serialize};

use crate::Page;
use crate::handlers::cookie;
use crate::handlers::extract::{Theme, Uids, serialize_uids};
use crate::handlers::html::make_error;
use crate::i18n::Lang;
use wastebin_core::db::{Database, write};

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub text: String,
    pub extension: Option<String>,
    pub expires: Option<String>,
    pub password: String,
    pub title: String,
    #[serde(rename = "burn-after-reading")]
    pub burn_after_reading: Option<String>,
}

impl From<Entry> for write::Entry {
    fn from(entry: Entry) -> Self {
        let burn_after_reading = entry.burn_after_reading.map(|s| s == "on");
        let password = (!entry.password.is_empty()).then_some(entry.password);
        let title = (!entry.title.is_empty()).then_some(entry.title);
        let expires = entry
            .expires
            .and_then(|expires| expires.parse::<NonZeroU32>().ok());

        Self {
            text: entry.text,
            extension: entry.extension,
            expires,
            burn_after_reading,
            uid: None,
            password,
            title,
        }
    }
}

pub async fn post<E: std::fmt::Debug>(
    State(page): State<Page>,
    State(db): State<Database>,
    jar: SignedCookieJar,
    uids: Option<Uids>,
    theme: Option<Theme>,
    lang: Lang,
    entry: Result<Form<Entry>, E>,
) -> Result<(SignedCookieJar, Redirect), impl IntoResponse> {
    let Ok(Form(entry)) = entry else {
        return Err(make_error(crate::Error::MalformedForm, page, theme, lang));
    };

    async {
        // Pick the existing primary uid (first in the cookie list) or mint a new one.
        // Re-set the cookie with the full list unchanged so claimed uids survive.
        let mut uids = uids.map(|Uids(uids)| uids).unwrap_or_default();
        let primary = match uids.first().copied() {
            Some(uid) => uid,
            None => {
                let uid = db.next_uid().await?;
                uids.push(uid);
                uid
            }
        };

        let mut entry: write::Entry = entry.into();
        entry.uid = Some(primary);

        let (id, entry) = db.insert(entry).await?;

        let url = {
            let url_path = id.to_url_path(&entry);
            if entry.burn_after_reading.unwrap_or(false) {
                format!("/burn/{url_path}")
            } else {
                format!("/{url_path}")
            }
        };

        let mut cookie = cookie("uid", serialize_uids(&uids));
        cookie.set_secure(true);

        Ok((jar.add(cookie), crate::redirect(&url)))
    }
    .await
    .map_err(|err| make_error(err, page, theme, lang))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{Client, StoreCookies};
    use reqwest::{StatusCode, header};
    use std::collections::HashMap;

    #[tokio::test]
    async fn insert() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let data = Entry {
            text: String::from("FooBarBaz"),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let location = res.headers().get("location").unwrap().to_str()?;

        let res = client
            .get(location)
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;

        assert_eq!(res.status(), StatusCode::OK);

        let header = res.headers().get(header::CONTENT_TYPE).unwrap();
        assert!(header.to_str().unwrap().contains("text/html"));

        let content = res.text().await?;
        assert!(content.contains("FooBarBaz"));

        let res = client
            .get(&format!("/raw{location}"))
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;

        assert_eq!(res.status(), StatusCode::OK);

        let header = res.headers().get(header::CONTENT_TYPE).unwrap();
        assert!(header.to_str().unwrap().contains("text/plain"));

        let content = res.text().await?;
        assert_eq!(content, "FooBarBaz");

        Ok(())
    }

    #[tokio::test]
    async fn insert_fail() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;

        let mut data = HashMap::new();
        data.insert("Hello", "World");

        let res = client.post_form().form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

        Ok(())
    }

    #[tokio::test]
    async fn insert_sets_uid_cookie() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(true)).await;
        let res = client.post_form().form(&Entry::default()).send().await?;
        let cookie = res.cookies().find(|cookie| cookie.name() == "uid").unwrap();
        assert_eq!(cookie.name(), "uid");
        assert!(cookie.value().len() > 40);
        assert_eq!(cookie.path().unwrap(), "/");
        assert!(cookie.http_only());
        assert!(cookie.same_site_strict());
        assert!(cookie.domain().is_none());
        assert!(cookie.expires().is_none());
        assert!(cookie.max_age().is_none());
        assert!(cookie.secure());

        Ok(())
    }
}
