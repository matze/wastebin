use std::num::NonZeroU32;

use axum::Json;
use axum::extract::State;
use axum_extra::extract::cookie::Key;
use serde::{Deserialize, Serialize};

use crate::errors::{Error, JsonErrorResponse};
use crate::handlers::extract::{sign_owner_token, verify_owner_token};
use wastebin_core::db::{Database, write};

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub text: String,
    pub extension: Option<String>,
    pub expires: Option<NonZeroU32>,
    pub burn_after_reading: Option<bool>,
    pub password: Option<String>,
    pub title: Option<String>,
    /// Optional server-signed owner token from a previous insert. When present
    /// and valid, the new paste reuses that token's uid instead of minting a
    /// new one, so all of a client's pastes share a single deletion identity.
    #[serde(default)]
    pub owner: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct RedirectResponse {
    pub path: String,
    /// Signed token claimed by visiting `<path>?owner=<token>`. Granting any
    /// browser that opens that URL the right to delete the paste.
    pub owner: String,
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

pub async fn post(
    State(db): State<Database>,
    State(key): State<Key>,
    Json(entry): Json<Entry>,
) -> Result<Json<RedirectResponse>, JsonErrorResponse> {
    // Reuse the uid encoded in a valid `owner` token so a client can group its
    // pastes under one identity; otherwise mint a fresh uid. A raw uid is never
    // trusted — only a server-signed token is accepted, and an invalid one falls
    // back to minting rather than failing the request.
    let uid = match entry
        .owner
        .as_deref()
        .and_then(|token| verify_owner_token(&key, token))
    {
        Some(uid) => uid,
        None => db.next_uid().await.map_err(Error::Database)?,
    };

    let mut entry: write::Entry = entry.into();
    entry.uid = Some(uid);

    let (id, entry) = db.insert(entry).await.map_err(Error::Database)?;
    let path = format!("/{}", id.to_url_path(&entry));
    let owner = sign_owner_token(&key, uid);

    Ok(Json::from(RedirectResponse { path, owner }))
}

#[cfg(test)]
mod tests {
    use crate::handlers::extract::PASSWORD_HEADER_NAME;
    use crate::test_helpers::{Client, StoreCookies};
    use reqwest::StatusCode;
    use wastebin_core::db::write::Entry;

    #[tokio::test]
    async fn insert() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;

        let entry = Entry {
            text: "FooBarBaz".to_string(),
            ..Default::default()
        };

        let res = client.post_json().json(&entry).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let payload = res.json::<super::RedirectResponse>().await?;

        let res = client.get(&format!("/raw{}", payload.path)).send().await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await?, "FooBarBaz");

        Ok(())
    }

    #[tokio::test]
    async fn insert_fail() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;

        let entry = "Hello World";

        let res = client.post_json().json(&entry).send().await?;
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

        Ok(())
    }

    #[tokio::test]
    async fn insert_returns_owner_token() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;

        let entry = Entry {
            text: "FooBarBaz".to_string(),
            ..Default::default()
        };

        let res = client.post_json().json(&entry).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let payload = res.json::<super::RedirectResponse>().await?;
        assert!(payload.path.starts_with('/'));
        assert!(!payload.owner.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn insert_reuses_owner_token_uid() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(true)).await;

        // First paste mints a uid and returns its token.
        let first = client
            .post_json()
            .json(&super::Entry {
                text: "first".to_string(),
                ..Default::default()
            })
            .send()
            .await?
            .json::<super::RedirectResponse>()
            .await?;

        // Second paste passes the token back to reuse the same uid.
        let second = client
            .post_json()
            .json(&super::Entry {
                text: "second".to_string(),
                owner: Some(first.owner.clone()),
                ..Default::default()
            })
            .send()
            .await?
            .json::<super::RedirectResponse>()
            .await?;

        // Same uid yields the same (deterministic) signed token.
        assert_eq!(first.owner, second.owner);

        // A single handoff claims the shared uid; both pastes become deletable.
        client
            .get(&first.path)
            .query(&[("owner", &first.owner)])
            .send()
            .await?;

        assert_eq!(
            client.delete(&first.path).send().await?.status(),
            StatusCode::OK
        );
        assert_eq!(
            client.delete(&second.path).send().await?.status(),
            StatusCode::OK
        );

        Ok(())
    }

    #[tokio::test]
    async fn insert_with_invalid_owner_mints_new_uid() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;

        let res = client
            .post_json()
            .json(&super::Entry {
                text: "FooBarBaz".to_string(),
                owner: Some("garbage".to_string()),
                ..Default::default()
            })
            .send()
            .await?;

        // Invalid token must not fail the insert, it falls back to a fresh uid.
        assert_eq!(res.status(), StatusCode::OK);

        let payload = res.json::<super::RedirectResponse>().await?;
        assert!(!payload.owner.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn delete_via_owner_handoff() -> Result<(), Box<dyn std::error::Error>> {
        // Browser-like client, keeps cookies across requests so the handoff cookie sticks for the
        // subsequent DELETE.
        let client = Client::new(StoreCookies(true)).await;

        let entry = Entry {
            text: "FooBarBaz".to_string(),
            ..Default::default()
        };

        let res = client.post_json().json(&entry).send().await?;
        let payload = res.json::<super::RedirectResponse>().await?;

        // Visit the magic link, server should redirect to clean URL with Set-Cookie.
        let res = client
            .get(&payload.path)
            .query(&[("owner", &payload.owner)])
            .send()
            .await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        assert_eq!(res.headers().get("location").unwrap(), &payload.path);
        assert!(
            res.headers()
                .get_all(reqwest::header::SET_COOKIE)
                .iter()
                .any(|v| v.to_str().unwrap_or("").starts_with("uid=")),
            "expected uid Set-Cookie on handoff",
        );

        // Cookie now in the jar, DELETE should succeed.
        let res = client.delete(&payload.path).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let res = client.get(&payload.path).send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }

    #[tokio::test]
    async fn invalid_owner_renders_normally() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;

        let entry = Entry {
            text: "FooBarBaz".to_string(),
            ..Default::default()
        };
        let res = client.post_json().json(&entry).send().await?;
        let payload = res.json::<super::RedirectResponse>().await?;

        let res = client
            .get(&payload.path)
            .query(&[("owner", "garbage")])
            .header(reqwest::header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            !res.headers().contains_key(reqwest::header::SET_COOKIE),
            "invalid token must not set a cookie",
        );

        Ok(())
    }

    #[tokio::test]
    async fn insert_encrypted() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let password = "SuperSecretPassword";

        let entry = Entry {
            text: "FooBarBaz".to_string(),
            password: Some(password.to_string()),
            ..Default::default()
        };

        let res = client.post_json().json(&entry).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let payload = res.json::<super::RedirectResponse>().await?;

        let res = client
            .get(&format!("/raw{}", payload.path))
            .header(PASSWORD_HEADER_NAME, password)
            .send()
            .await?;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await?, "FooBarBaz");

        Ok(())
    }
}
