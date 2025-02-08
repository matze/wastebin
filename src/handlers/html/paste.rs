use crate::cache::Key;
use crate::crypto::Password;
use crate::db::read::Entry;
use crate::handlers::html::{make_error, ErrorResponse, PasswordInput};
use crate::highlight::Html;
use crate::{Cache, Database, Error, Highlighter, Page};
use askama::Template;
use axum::extract::{Form, Path, State};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::SignedCookieJar;
use http::{header, HeaderMap};
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
    headers: HeaderMap,
    jar: SignedCookieJar,
    form: Option<Form<PasswordForm>>,
) -> Result<Response, ErrorResponse> {
    async {
        let password = form.map(|form| Password::from(form.password.as_bytes().to_vec()));
        let key: Key = id.parse()?;

        match db.get(key.id, password.clone()).await {
            Ok(entry) => {
                if entry.must_be_deleted {
                    db.delete(key.id).await?;
                }

                let accept_html = headers
                    .get(header::ACCEPT)
                    .and_then(|value| value.to_str().ok())
                    .map_or(false, |value| value.contains("text/html"));

                if accept_html {
                    return Ok(get_html(
                        page.clone(),
                        cache,
                        highlighter,
                        key,
                        entry,
                        jar,
                        password.is_some(),
                    )
                    .await
                    .into_response());
                }

                Ok(entry.text.into_response())
            }
            Err(Error::NoPassword) => Ok(PasswordInput {
                page: page.clone(),
                id,
            }
            .into_response()),
            Err(err) => Err(err),
        }
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

async fn get_html(
    page: Page,
    cache: Cache,
    highlighter: Highlighter,
    key: Key,
    entry: Entry,
    jar: SignedCookieJar,
    is_protected: bool,
) -> Result<impl IntoResponse, ErrorResponse> {
    async {
        let can_delete = jar
            .get("uid")
            .map(|cookie| cookie.value().parse::<i64>())
            .transpose()
            .map_err(|err| Error::CookieParsing(err.to_string()))?
            .zip(entry.uid)
            .is_some_and(|(user_uid, owner_uid)| user_uid == owner_uid);

        if let Some(html) = cache.get(&key) {
            tracing::trace!(?key, "found cached item");

            let title = entry.title.unwrap_or_default();
            return Ok(Paste::new(key, html, can_delete, title, page.clone()).into_response());
        }

        // TODO: turn this upside-down, i.e. cache it but only return a cached version if we were able
        // to decrypt the content. Highlighting is probably still much slower than decryption.
        let can_be_cached = !entry.must_be_deleted;
        let ext = key.ext.clone();
        let title = entry.title.clone().unwrap_or_default();
        let html = highlighter.highlight(entry, ext).await?;

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
