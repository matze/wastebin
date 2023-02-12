use crate::db::InsertEntry;
use crate::id::Id;
use crate::{AppState, Error};
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub text: String,
    pub extension: Option<String>,
    pub expires: Option<u32>,
    pub burn_after_reading: Option<bool>,
}

#[derive(Deserialize, Serialize)]
pub struct RedirectResponse {
    pub path: String,
}

#[derive(Serialize)]
pub struct ErrorPayload {
    pub message: String,
}

pub type ErrorResponse = (StatusCode, Json<ErrorPayload>);

impl From<Entry> for InsertEntry {
    fn from(entry: Entry) -> Self {
        Self {
            text: entry.text,
            extension: entry.extension,
            expires: entry.expires,
            burn_after_reading: entry.burn_after_reading,
            uid: None,
        }
    }
}

impl From<Error> for ErrorResponse {
    fn from(err: Error) -> Self {
        let payload = Json::from(ErrorPayload {
            message: err.to_string(),
        });

        (err.into(), payload)
    }
}

pub async fn insert(
    state: State<AppState>,
    Json(entry): Json<Entry>,
) -> Result<Json<RedirectResponse>, ErrorResponse> {
    let id: Id = tokio::task::spawn_blocking(|| {
        let mut rng = rand::thread_rng();
        rng.gen::<u32>()
    })
    .await
    .map_err(Error::from)?
    .into();

    let entry: InsertEntry = entry.into();
    let path = id.to_url_path(&entry);

    state.db.insert(id, entry).await?;

    Ok(Json::from(RedirectResponse { path }))
}
