use crate::cache::Key;
use crate::crypto::Password;
use crate::db::read::{Data, Entry};
use crate::handlers::html::{make_error, ErrorResponse, PasswordInput};
use crate::highlight::Html;
use crate::{Cache, Database, Error, Highlighter, Page};
use askama::Template;
use axum::extract::{Form, Path, State};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::SignedCookieJar;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct PasswordForm {
    password: String,
}

/// Paste view showing the formatted paste.
#[derive(Template)]
#[template(path = "formatted.html")]
pub struct Paste {
    page: Page,
    key: Key,
    can_delete: bool,
    html: String,
    title: String,
}

#[expect(clippy::too_many_arguments)]
pub async fn get(
    State(cache): State<Cache>,
    State(page): State<Page>,
    State(db): State<Database>,
    State(highlighter): State<Highlighter>,
    Path(id): Path<String>,
    jar: SignedCookieJar,
    form: Option<Form<PasswordForm>>,
) -> Result<Response, ErrorResponse> {
    async {
        let password = form.map(|form| Password::from(form.password.as_bytes().to_vec()));
        let key: Key = id.parse()?;

        let (data, can_be_cached) = match db.get(key.id, password.clone()).await {
            Ok(Entry::Regular(data)) => (data, true),
            Ok(Entry::Burned(data)) => (data, false),
            Ok(Entry::Expired) => return Err(Error::NotFound),
            Err(Error::NoPassword) => {
                return Ok(PasswordInput {
                    page: page.clone(),
                    id,
                }
                .into_response())
            }
            Err(err) => return Err(err),
        };

        Ok(get_html(
            page.clone(),
            cache,
            highlighter,
            key,
            data,
            can_be_cached,
            jar,
            password.is_some(),
        )
        .await
        .into_response())
    }
    .await
    .map_err(|err| make_error(err, page))
}

impl Paste {
    /// Construct new paste view from cache `key` and paste `html`.
    pub fn new(key: Key, html: Html, can_delete: bool, title: String, page: Page) -> Self {
        let html = html.into_inner();

        Self {
            page,
            key,
            can_delete,
            html,
            title,
        }
    }
}

#[expect(clippy::too_many_arguments)]
async fn get_html(
    page: Page,
    cache: Cache,
    highlighter: Highlighter,
    key: Key,
    data: Data,
    can_be_cached: bool,
    jar: SignedCookieJar,
    is_protected: bool,
) -> Result<impl IntoResponse, ErrorResponse> {
    async {
        let can_delete = jar
            .get("uid")
            .map(|cookie| cookie.value().parse::<i64>())
            .transpose()
            .map_err(|err| Error::CookieParsing(err.to_string()))?
            .zip(data.uid)
            .is_some_and(|(user_uid, owner_uid)| user_uid == owner_uid);

        if let Some(html) = cache.get(&key) {
            tracing::trace!(?key, "found cached item");

            let title = data.title.unwrap_or_default();
            return Ok(Paste::new(key, html, can_delete, title, page.clone()).into_response());
        }

        // TODO: turn this upside-down, i.e. cache it but only return a cached version if we were able
        // to decrypt the content. Highlighting is probably still much slower than decryption.
        let ext = key.ext.clone();
        let title = data.title.clone().unwrap_or_default();
        let html = highlighter.highlight(data, ext).await?;

        if can_be_cached && !is_protected {
            tracing::trace!(?key, "cache item");
            cache.put(key.clone(), html.clone());
        }

        Ok(Paste::new(key, html, can_delete, title, page.clone()).into_response())
    }
    .await
    .map_err(|err| make_error(err, page))
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::Client;
    use reqwest::StatusCode;

    #[tokio::test]
    async fn unknown_paste() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new().await;

        let res = client.get("/000000").send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }
}
