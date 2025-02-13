use crate::AppState;
use axum::body::Body;
use axum::extract::{Form, Json, State};
use axum::http::header::HeaderMap;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::RequestExt;
use axum_extra::extract::cookie::SignedCookieJar;
use axum_extra::headers;
use axum_extra::headers::HeaderMapExt;

pub async fn insert(
    state: State<AppState>,
    jar: SignedCookieJar,
    headers: HeaderMap,
    request: Request<Body>,
) -> Result<Response, Response> {
    let content_type = headers
        .typed_get::<headers::ContentType>()
        .ok_or_else(|| StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response())?;

    if content_type == headers::ContentType::form_url_encoded() {
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

        let entry: Form<form::Entry> = request
            .extract()
            .await
            .map_err(IntoResponse::into_response)?;

        Ok(form::insert(state, jar, entry, is_https)
            .await
            .into_response())
    } else if content_type == headers::ContentType::json() {
        let entry: Json<json::Entry> = request
            .extract()
            .await
            .map_err(IntoResponse::into_response)?;

        Ok(json::insert(state, entry).await.into_response())
    } else {
        Err(StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response())
    }
}

pub mod form {
    use crate::db::write;
    use crate::handlers::html::make_error;
    use crate::id::Id;
    use crate::{AppState, Error};
    use axum::extract::{Form, State};
    use axum::response::{IntoResponse, Redirect};
    use axum_extra::extract::cookie::{Cookie, SameSite, SignedCookieJar};
    use rand::Rng;
    use serde::{Deserialize, Serialize};
    use std::num::NonZeroU32;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Entry {
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

    pub async fn insert(
        state: State<AppState>,
        jar: SignedCookieJar,
        Form(entry): Form<Entry>,
        is_https: bool,
    ) -> Result<(SignedCookieJar, Redirect), impl IntoResponse> {
        async {
            let id: Id = tokio::task::spawn_blocking(|| {
                let mut rng = rand::thread_rng();
                rng.gen::<u32>()
            })
            .await
            .map_err(Error::from)?
            .into();

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

            let mut url = id.to_url_path(&entry);

            if entry.burn_after_reading.unwrap_or(false) {
                url = format!("burn/{url}");
            }

            if let Some(max_exp) = state.max_expiration {
                entry.expires = entry
                    .expires
                    .map_or_else(|| Some(max_exp), |value| Some(value.min(max_exp)));
            }

            state.db.insert(id, entry).await?;
            let url = format!("/{url}");

            let cookie = Cookie::build(("uid", uid.to_string()))
                .http_only(true)
                .secure(is_https)
                .same_site(SameSite::Strict)
                .build();

            Ok((jar.add(cookie), Redirect::to(&url)))
        }
        .await
        .map_err(|err| make_error(err, state.page.clone()))
    }

    #[cfg(test)]
    mod tests {
        use crate::test_helpers::Client;
        use reqwest::{header, StatusCode};
        use std::collections::HashMap;

        #[tokio::test]
        async fn insert() -> Result<(), Box<dyn std::error::Error>> {
            let client = Client::new().await;

            let data = super::Entry {
                text: "FooBarBaz".to_string(),
                extension: Some("rs".to_string()),
                expires: "0".to_string(),
                password: "".to_string(),
                title: "".to_string(),
            };

            let res = client.post("/").form(&data).send().await?;
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
            let client = Client::new().await;

            let mut data = HashMap::new();
            data.insert("Hello", "World");

            let res = client.post("/").form(&data).send().await?;
            assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

            Ok(())
        }
    }
}

mod json {
    use crate::db::write;
    use crate::errors::{Error, JsonErrorResponse};
    use crate::id::Id;
    use crate::AppState;
    use axum::extract::State;
    use axum::Json;
    use rand::Rng;
    use serde::{Deserialize, Serialize};
    use std::num::NonZeroU32;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Entry {
        pub text: String,
        pub extension: Option<String>,
        pub expires: Option<NonZeroU32>,
        pub burn_after_reading: Option<bool>,
        pub password: Option<String>,
        pub title: Option<String>,
    }

    #[derive(Deserialize, Serialize)]
    pub struct RedirectResponse {
        pub path: String,
    }

    impl From<Entry> for write::Entry {
        fn from(entry: Entry) -> Self {
            Self {
                text: entry.text,
                extension: entry.extension,
                expires: entry.expires,
                burn_after_reading: entry.burn_after_reading,
                uid: None,
                password: entry.password,
                title: entry.title,
            }
        }
    }

    pub async fn insert(
        state: State<AppState>,
        Json(entry): Json<Entry>,
    ) -> Result<Json<RedirectResponse>, JsonErrorResponse> {
        let id: Id = tokio::task::spawn_blocking(|| {
            let mut rng = rand::thread_rng();
            rng.gen::<u32>()
        })
        .await
        .map_err(Error::from)?
        .into();

        let mut entry: write::Entry = entry.into();

        if let Some(max_exp) = state.max_expiration {
            entry.expires = entry
                .expires
                .map_or_else(|| Some(max_exp), |value| Some(value.min(max_exp)));
        }

        let path = format!("/raw/{}", id.to_url_path(&entry));
        state.db.insert(id, entry).await?;

        Ok(Json::from(RedirectResponse { path }))
    }

    #[cfg(test)]
    mod tests {
        use crate::db::write::Entry;
        use crate::test_helpers::Client;
        use reqwest::StatusCode;

        #[tokio::test]
        async fn insert() -> Result<(), Box<dyn std::error::Error>> {
            let client = Client::new().await;

            let entry = Entry {
                text: "FooBarBaz".to_string(),
                ..Default::default()
            };

            let res = client.post("/").json(&entry).send().await?;
            assert_eq!(res.status(), StatusCode::OK);

            let payload = res.json::<super::RedirectResponse>().await?;

            let res = client.get(&payload.path).send().await?;
            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(res.text().await?, "FooBarBaz");

            Ok(())
        }

        #[tokio::test]
        async fn insert_fail() -> Result<(), Box<dyn std::error::Error>> {
            let client = Client::new().await;

            let entry = "Hello World";

            let res = client.post("/").json(&entry).send().await?;
            assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

            Ok(())
        }

        #[tokio::test]
        async fn insert_encrypted() -> Result<(), Box<dyn std::error::Error>> {
            let client = Client::new().await;
            let password = "SuperSecretPassword";

            let entry = Entry {
                text: "FooBarBaz".to_string(),
                password: Some(password.to_string()),
                ..Default::default()
            };

            let res = client.post("/").json(&entry).send().await?;
            assert_eq!(res.status(), StatusCode::OK);

            let payload = res.json::<super::RedirectResponse>().await?;
            println!("{}", payload.path);

            let res = client
                .get(&payload.path)
                .header("Wastebin-Password", password)
                .send()
                .await?;

            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(res.text().await?, "FooBarBaz");

            Ok(())
        }
    }
}
