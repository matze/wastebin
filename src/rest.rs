use crate::db::Database;
use crate::id::Id;
use crate::{Entry, Error};
use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use rand::Rng;
use serde::Serialize;

#[derive(Serialize)]
struct ErrorPayload {
    message: String,
}

#[derive(Serialize)]
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

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn insert(
    Json(entry): Json<Entry>,
    db: Extension<Database>,
) -> Result<Json<RedirectResponse>, ErrorResponse> {
    let id: Id = tokio::task::spawn_blocking(|| {
        let mut rng = rand::thread_rng();
        rng.gen::<u32>()
    })
    .await
    .map_err(Error::from)?
    .into();

    let path = id.to_url_path(&entry);

    db.insert(id, entry).await?;
    Ok(Json::from(RedirectResponse { path }))
}

async fn raw(Path(id): Path<String>, db: Extension<Database>) -> Result<String, ErrorResponse> {
    Ok(db.get(Id::try_from(id.as_str())?).await?.text)
}

pub fn routes() -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/entries", post(insert))
        .route("/api/entries/:id", get(raw))
}
