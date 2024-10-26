use std::num::NonZeroU32;

use crate::db::write;
use crate::env::BASE_PATH;
use crate::errors::JsonErrorResponse;
use crate::AppState;
use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub text: String,
    pub extension: Option<String>,
    pub expires: Option<NonZeroU32>,
    pub burn_after_reading: Option<bool>,
    pub password: Option<String>,
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
        }
    }
}

pub async fn insert(
    state: State<AppState>,
    Json(entry): Json<Entry>,
) -> Result<Json<RedirectResponse>, JsonErrorResponse> {
    let mut entry: write::Entry = entry.into();

    if let Some(max_exp) = state.max_expiration {
        entry.expires = entry
            .expires
            .map_or_else(|| Some(max_exp), |value| Some(value.min(max_exp)));
    }

    let extension = entry.extension.clone();

    let id = state.db.insert(entry).await?;
    let url = id.to_url_path(extension.as_deref());
    let path = BASE_PATH.join(&url);

    Ok(Json::from(RedirectResponse { path }))
}
