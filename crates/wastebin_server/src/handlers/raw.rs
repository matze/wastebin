use crate::cache::Key;
use crate::handlers::extract::{Password, Theme};
use crate::handlers::html::{ErrorResponse, PasswordInput, make_error};
use crate::{Database, Page};
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use wastebin_core::db;
use wastebin_core::db::read::Entry;

/// GET handler for raw content of a paste.
pub async fn get(
    Path(id): Path<String>,
    State(db): State<Database>,
    State(page): State<Page>,
    theme: Option<Theme>,
    password: Option<Password>,
) -> Result<Response, ErrorResponse> {
    async {
        let password = password.map(|Password(password)| password);
        let key: Key = id.parse()?;

        match db.get(key.id, password).await {
            Ok(Entry::Regular(data) | Entry::Burned(data)) => Ok(data.text.into_response()),
            Err(db::Error::NoPassword) => Ok(PasswordInput {
                page: page.clone(),
                theme: theme.clone(),
                id: key.id.to_string(),
            }
            .into_response()),
            Err(err) => Err(err.into()),
        }
    }
    .await
    .map_err(|err| make_error(err, page, theme))
}
