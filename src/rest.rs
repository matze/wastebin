use crate::cache::Layer;
use crate::id::Id;
use crate::{Entry, Error, Router};
use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Extension, Json};
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct ErrorPayload {
    message: String,
}

#[derive(Deserialize, Serialize)]
struct RedirectResponse {
    path: String,
}

type ErrorResponse = (StatusCode, Json<ErrorPayload>);

impl From<Error> for ErrorResponse {
    fn from(err: Error) -> Self {
        let payload = Json::from(ErrorPayload {
            message: err.to_string(),
        });

        (err.into(), payload)
    }
}

#[allow(clippy::unused_async)]
async fn health() -> StatusCode {
    StatusCode::OK
}

async fn insert(
    Json(entry): Json<Entry>,
    layer: Extension<Layer>,
) -> Result<Json<RedirectResponse>, ErrorResponse> {
    let id: Id = tokio::task::spawn_blocking(|| {
        let mut rng = rand::thread_rng();
        rng.gen::<u32>()
    })
    .await
    .map_err(Error::from)?
    .into();

    let path = id.to_url_path(&entry);

    layer.insert(id, entry).await?;
    Ok(Json::from(RedirectResponse { path }))
}

async fn raw(Path(id): Path<String>, layer: Extension<Layer>) -> Result<String, ErrorResponse> {
    Ok(layer.get(Id::try_from(id.as_str())?).await?.text)
}

async fn delete(Path(id): Path<String>, layer: Extension<Layer>) -> Result<(), ErrorResponse> {
    let id = Id::try_from(id.as_str())?;
    let entry = layer.get(id).await?;

    if entry.seconds_since_creation > 60 {
        Err(Error::DeletionTimeExpired)?
    }

    layer.delete(id).await?;
    Ok(())
}

pub fn routes() -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/entries", post(insert))
        .route("/api/entries/:id", get(raw).delete(delete))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{make_app, Client};
    use crate::Entry;
    use http::StatusCode;

    #[tokio::test]
    async fn health() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(make_app()?);

        let res = client.get("/api/health").send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        Ok(())
    }

    #[tokio::test]
    async fn entries() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(make_app()?);

        let entry = Entry {
            text: "FooBarBaz".to_string(),
            ..Default::default()
        };

        let res = client.post("/api/entries").json(&entry).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let payload = res.json::<RedirectResponse>().await?;
        let path = format!("/api/entries{}", payload.path);
        println!("{path}");

        let res = client.get(&path).send().await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await?, "FooBarBaz");

        let res = client.delete(&path).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let res = client.get(&path).send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }
}
