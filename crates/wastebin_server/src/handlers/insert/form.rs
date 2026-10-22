use std::num::NonZeroU32;

use axum::extract::{Form, State};
use axum::response::{IntoResponse, Redirect};
use axum_extra::extract::cookie::SignedCookieJar;
use serde::{Deserialize, Serialize};

use crate::Page;
use crate::cache::valid_extension;
use crate::handlers::extract::{Theme, Uids};
use crate::handlers::html::make_error;
use crate::handlers::uid_cookie;
use crate::i18n::Lang;
use wastebin_core::db::{Database, write};
use wastebin_core::id;

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

impl TryFrom<Entry> for write::Entry {
    type Error = crate::Error;

    fn try_from(entry: Entry) -> Result<Self, Self::Error> {
        let burn_after_reading = entry.burn_after_reading.map(|s| s == "on");
        let password = (!entry.password.is_empty()).then_some(entry.password);
        let title = (!entry.title.is_empty()).then_some(entry.title);
        // `0` is how the form spells "never expires". Anything else that is not a number is a
        // malformed request, not another way to ask for a paste that is kept forever.
        let expires = entry
            .expires
            .map(|expires| {
                expires
                    .parse::<u32>()
                    .map(NonZeroU32::new)
                    .map_err(|_| crate::Error::MalformedForm)
            })
            .transpose()?
            .flatten();

        Ok(Self {
            text: entry.text,
            extension: entry.extension,
            expires,
            burn_after_reading,
            uid: None,
            password,
            title,
        })
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

    if let Some(extension) = &entry.extension
        && !valid_extension(extension)
    {
        return Err(make_error(
            crate::Error::Id(id::Error::IllegalCharacters),
            page,
            theme,
            lang,
        ));
    }

    let mut entry: write::Entry = match entry.try_into() {
        Ok(entry) => entry,
        Err(err) => return Err(make_error(err, page, theme, lang)),
    };
    let mut uids = uids.map(|Uids(uids)| uids).unwrap_or_default();

    async {
        // Pick the existing primary uid (first in the cookie list) or mint a new one.
        // Re-set the cookie with the full list unchanged so claimed uids survive.
        let primary = match uids.first().copied() {
            Some(uid) => uid,
            None => {
                let uid = db.next_uid().await?;
                uids.push(uid);
                uid
            }
        };

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

        let cookie = uid_cookie(&uids);

        Ok((jar.add(cookie), Redirect::to(&url)))
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
    async fn unparsable_expiration_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;

        let data = Entry {
            text: String::from("FooBarBaz"),
            expires: Some(String::from("garbage")),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

        Ok(())
    }

    #[tokio::test]
    async fn zero_expiration_still_means_never() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;

        let data = Entry {
            text: String::from("FooBarBaz"),
            expires: Some(String::from("0")),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        Ok(())
    }

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
    async fn insert_rejects_invalid_extension() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let data = Entry {
            text: String::from("FooBarBaz"),
            extension: Some(String::from("\"><img src=x>")),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        Ok(())
    }

    #[tokio::test]
    async fn insert_accepts_dotted_extension() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let data = Entry {
            text: String::from("FooBarBaz"),
            // One of the picker's dotted extensions.
            extension: Some(String::from("CMakeLists.txt")),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let location = res.headers().get("location").unwrap().to_str()?;
        assert!(location.ends_with(".CMakeLists.txt"));

        let res = client
            .get(location)
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;
        assert_eq!(res.status(), StatusCode::OK);

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
        assert!(cookie.same_site_lax());
        assert!(cookie.domain().is_none());
        assert!(cookie.expires().is_none());
        assert!(cookie.max_age().is_none());
        assert!(cookie.secure());

        Ok(())
    }
}
