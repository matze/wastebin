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

    let entry: write::Entry = entry.into();
    let path = format!("/{}", id.to_url_path(&entry));
    state.db.insert(id, entry).await?;

    Ok(Json::from(RedirectResponse { path }))
}

#[cfg(test)]
mod tests {
    use crate::db::write::Entry;
    use crate::handlers::extract::PASSWORD_HEADER_NAME;
    use crate::test_helpers::Client;
    use reqwest::StatusCode;

    #[tokio::test]
    async fn insert() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new().await;

        let entry = Entry {
            text: "FooBarBaz".to_string(),
            ..Default::default()
        };

        let res = client.post("/api").json(&entry).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let payload = res.json::<super::RedirectResponse>().await?;

        let res = client.get(&format!("/raw{}", payload.path)).send().await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await?, "FooBarBaz");

        Ok(())
    }

    #[tokio::test]
    async fn insert_fail() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new().await;

        let entry = "Hello World";

        let res = client.post("/api").json(&entry).send().await?;
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

        let res = client.post("/api").json(&entry).send().await?;
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
