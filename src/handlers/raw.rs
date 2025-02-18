use crate::cache::Key;
use crate::db::read::Entry;
use crate::handlers::extract::{Password, Theme};
use crate::handlers::html::{make_error, ErrorResponse, PasswordInput};
use crate::{Database, Error, Page};
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};

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
            Ok(Entry::Expired) => Err(Error::NotFound),
            Err(Error::NoPassword) => Ok(PasswordInput {
                page: page.clone(),
                theme: theme.clone(),
                id: key.id.to_string(),
            }
            .into_response()),
            Err(err) => Err(err),
        }
    }
    .await
    .map_err(|err| make_error(err, page, theme))
}
