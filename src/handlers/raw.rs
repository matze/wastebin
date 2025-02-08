use crate::cache::Key;
use crate::crypto::Password;
use crate::handlers::html::{make_error, ErrorResponse, PasswordInput};
use crate::{Database, Error, Page};
use axum::extract::{Form, Path, State};
use axum::response::{IntoResponse, Response};
use http::HeaderMap;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct PasswordForm {
    password: String,
}

/// GET handler for raw content of a paste.
pub async fn raw(
    Path(id): Path<String>,
    State(db): State<Database>,
    State(page): State<Page>,
    headers: HeaderMap,
    form: Option<Form<PasswordForm>>,
) -> Result<Response, ErrorResponse> {
    async {
        let password = form
            .map(|form| form.password.clone())
            .or_else(|| {
                headers
                    .get("Wastebin-Password")
                    .and_then(|header| header.to_str().ok().map(std::string::ToString::to_string))
            })
            .map(|password| Password::from(password.as_bytes().to_vec()));
        let key: Key = id.parse()?;

        match db.get(key.id, password.clone()).await {
            Ok(entry) => {
                if entry.must_be_deleted {
                    db.delete(key.id).await?;
                }

                Ok(entry.text.into_response())
            }
            Err(Error::NoPassword) => Ok(PasswordInput {
                page: page.clone(),
                id: key.id.to_string(),
            }
            .into_response()),
            Err(err) => Err(err),
        }
    }
    .await
    .map_err(|err| make_error(err, page))
}
