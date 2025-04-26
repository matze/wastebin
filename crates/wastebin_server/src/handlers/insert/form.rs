use crate::Page;
use crate::handlers::extract::{Theme, Uid};
use crate::handlers::html::make_error;
use axum::extract::{State, Multipart};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect};
use axum_extra::extract::cookie::{Cookie, SameSite, SignedCookieJar};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use wastebin_core::db::{Database, write};
use wastebin_core::id::Id;
use std::str;

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

async fn multipart_to_entry(mut entry: Multipart) -> Result<Entry, crate::Error> {
    let mut title = "".to_string();
    let mut text: String = "".to_string();
    let mut password = "".to_string();
    let mut burn_after_reading = None;
    let mut extension = None;
    let mut expires = None;

    while let Some(field) = entry.next_field().await.unwrap() {
        let name = String::from(
            field
            .name()
            .ok_or(crate::Error::MalformedForm)?
            );
        let data = String::from(
            str::from_utf8(
                dbg!(&field
                .bytes()
                .await
                .map_err(|_| crate::Error::MalformedForm)?)
                )
            .map_err(|_| crate::Error::MalformedForm)?
            );

        match name.as_str() {
            "title" => title = data,
            "text" => text = data,
            "password" => password = data,
            "burn_after_reading" => burn_after_reading = Some(data),
            "extension" => extension = Some(data),
            "expires" => expires = Some(data),
            _ => {}
        }
    }


    Ok(Entry {
        text,
        extension,
        expires,
        burn_after_reading,
        password,
        title,
    })
}

pub async fn post<E: std::fmt::Debug>(
    State(page): State<Page>,
    State(db): State<Database>,
    jar: SignedCookieJar,
    headers: HeaderMap,
    uid: Option<Uid>,
    theme: Option<Theme>,
    entry: Result<Multipart, E>,
) -> Result<(SignedCookieJar, Redirect), impl IntoResponse> {
    let Ok(entry) = entry else {
        return Err(make_error(crate::Error::MalformedForm, page, theme));
    };

    // TODO: think about something more appropriate because those headers might be all messed up
    // and yet we still have a proper TLS connection.
    let is_https = headers
        .get(http::header::HOST)
        .zip(headers.get(http::header::ORIGIN))
        .and_then(|(host, origin)| host.to_str().ok().zip(origin.to_str().ok()))
        .and_then(|(host, origin)| {
            origin
                .strip_prefix("https://")
                .map(|origin| origin.starts_with(host))
        })
        .unwrap_or(false);

    async {
        // Use cookie uid or generate a new one.
        let uid = if let Some(Uid(uid)) = uid {
            uid
        } else {
            db.next_uid().await?
        };

        let multipart_entry: Entry = multipart_to_entry(entry).await?;
        let mut entry: write::Entry = multipart_entry.into();
        entry.uid = Some(uid);

        let id = Id::rand();
        let mut url = id.to_url_path(&entry);

        if entry.burn_after_reading.unwrap_or(false) {
            url = format!("burn/{url}");
        }

        db.insert(id, entry).await?;
        let url = format!("/{url}");

        let cookie = Cookie::build(("uid", uid.to_string()))
            .http_only(true)
            .secure(is_https)
            .same_site(SameSite::Strict)
            .build();

        Ok((jar.add(cookie), Redirect::to(&url)))
    }
    .await
    .map_err(|err| make_error(err, page, theme))
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
        assert!(cookie.path().is_none());
        assert!(cookie.http_only());
        assert!(cookie.same_site_strict());
        assert!(cookie.domain().is_none());
        assert!(cookie.expires().is_none());
        assert!(cookie.max_age().is_none());
        assert!(!cookie.secure());

        Ok(())
    }
}
