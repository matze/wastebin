use crate::cache::Key;
use crate::handlers::extract::{Theme, Uid};
use crate::handlers::html::{ErrorResponse, PasswordInput, make_error};
use crate::highlight::Html;
use crate::{Cache, Database, Highlighter, Page};
use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Form, Path, State};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use wastebin_core::crypto::Password;
use wastebin_core::db;
use wastebin_core::db::read::Entry;

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
        let key: Key = id.parse()?;

        let (data, is_available) = match db.get(key.id, password.clone()).await {
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

        let can_be_deleted = uid
            .zip(data.uid)
            .is_some_and(|(Uid(user_uid), owner_uid)| user_uid == owner_uid);

        let title = data.title.clone();

        let html = if let Some(html) = cache.get(&key) {
            tracing::trace!(?key, "found cached item");

            html
        } else {
            let html = highlighter.highlight(data, key.ext.clone()).await?;

            if is_available && password.is_none() {
                tracing::trace!(?key, "cache item");
                cache.put(key.clone(), html.clone());
            }

            html
        };

        Ok(Paste::new(
            key,
            html,
            theme.clone(),
            can_be_deleted,
            is_available,
            title,
            page.clone(),
        )
        .into_response())
    }
    .await
    .map_err(|err| make_error(err, page, theme))
}

impl Paste {
    /// Construct new paste view from cache `key` and paste `html`.
    pub fn new(
        key: Key,
        html: Html,
        theme: Option<Theme>,
        can_delete: bool,
        is_available: bool,
        title: Option<String>,
        page: Page,
    ) -> Self {
        let html = html.into_inner();

        Self {
            page,
            key,
            theme,
            can_delete,
            is_available,
            html,
            title,
        }
    }
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
