use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Form, Path, State};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::cache::Key;
use crate::handlers::extract::{Theme, Uid};
use crate::handlers::html::{ErrorResponse, PasswordInput, make_error};
use crate::{Cache, Database, Highlighter, Page};
use wastebin_core::crypto::Password;
use wastebin_core::db;
use wastebin_core::db::read::{Data, Entry, Metadata};
use wastebin_core::expiration::Expiration;

#[derive(Deserialize, Debug)]
pub(crate) struct PasswordForm {
    password: String,
}

/// Paste view showing the formatted paste.
#[derive(Template, WebTemplate)]
#[template(path = "formatted.html")]
pub(crate) struct Paste {
    page: Page,
    key: Key,
    theme: Option<Theme>,
    can_delete: bool,
    /// If the paste still in the database and can be fetched with another request.
    is_available: bool,
    /// Expiration in case it was set.
    expiration: Option<Expiration>,
    html: String,
    title: Option<String>,
}

#[expect(clippy::too_many_arguments)]
pub async fn get<E>(
    State(cache): State<Cache>,
    State(page): State<Page>,
    State(db): State<Database>,
    State(highlighter): State<Highlighter>,
    Path(id): Path<String>,
    uid: Option<Uid>,
    theme: Option<Theme>,
    form: Result<Form<PasswordForm>, E>,
) -> Result<Response, ErrorResponse> {
    async {
        let password = form
            .ok()
            .map(|form| Password::from(form.password.as_bytes().to_vec()));
        let no_password = password.is_none();
        let key: Key = id.parse()?;

        let (data, is_available) = match db.get(key.id, password).await {
            Ok(Entry::Regular(data)) => (data, true),
            Ok(Entry::Burned(data)) => (data, false),
            Err(db::Error::NoPassword) => {
                return Ok(PasswordInput {
                    page: page.clone(),
                    theme: theme.clone(),
                    id,
                }
                .into_response());
            }
            Err(err) => return Err(err.into()),
        };

        let Data { text, metadata } = data;
        let Metadata {
            uid: owner_uid,
            title,
            expiration,
        } = metadata;

        let can_delete = uid
            .zip(owner_uid)
            .is_some_and(|(Uid(user_uid), owner_uid)| user_uid == owner_uid);

        let html = if let Some(html) = cache.get(&key) {
            tracing::trace!(?key, "found cached item");
            html.into_inner()
        } else {
            let html = highlighter.highlight(text, key.ext.clone()).await?;

            if is_available && no_password {
                tracing::trace!(?key, "cache item");
                cache.put(key.clone(), html.clone());
            }

            html.into_inner()
        };

        let paste = Paste {
            page: page.clone(),
            key,
            theme: theme.clone(),
            can_delete,
            is_available,
            expiration,
            html,
            title,
        };

        Ok(paste.into_response())
    }
    .await
    .map_err(|err| make_error(err, page, theme))
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::{Client, StoreCookies};
    use reqwest::StatusCode;

    #[tokio::test]
    async fn unknown_paste() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;

        let res = client.get("/000000").send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }
}
