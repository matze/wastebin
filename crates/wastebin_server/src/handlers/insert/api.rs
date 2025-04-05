use crate::errors::{Error, JsonErrorResponse};
use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use wastebin_core::db::{Database, write};
use wastebin_core::id::Id;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub text: String,
    pub extension: Option<String>,
    pub expires: Option<NonZeroU32>,
    pub burn_after_reading: Option<bool>,
    pub password: Option<String>,
    pub title: Option<String>,
    pub human_readable: Option<bool>,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct RedirectResponse {
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

pub async fn post(
    State(db): State<Database>,
    Json(entry): Json<Entry>,
) -> Result<Json<RedirectResponse>, JsonErrorResponse> {
    let db_entry: write::Entry = entry.clone().into();
    let mut id;

    loop {
        id = if entry
            .human_readable
            .is_some_and(|human_readabole| human_readabole)
        {
            Id::rand_human_readable()
        } else {
            Id::rand()
        };
        if let Ok(_) = db
            .insert(id.clone(), db_entry.clone())
            .await
            .map_err(Error::Database)
        {
            break;
        }
    }

    let path = format!("/{}", id.to_url_path(&db_entry));

    Ok(Json::from(RedirectResponse { path }))
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
