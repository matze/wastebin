use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Form, Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::SignedCookieJar;
use axum_extra::extract::cookie::Key as CookieKey;
use serde::Deserialize;

use crate::cache::{Key, Mode};
use crate::handlers::cookie;
use crate::handlers::extract::{Theme, Uids, serialize_uids, verify_owner_token};
use crate::handlers::html::{BurnConfirmation, ErrorResponse, PasswordInput, make_error};
use crate::i18n::Lang;
use crate::{Cache, Database, Highlighter, Page};
use wastebin_core::crypto::Password;
use wastebin_core::db;
use wastebin_core::db::read::{Data, Entry, Metadata};
use wastebin_core::expiration::Expiration;

/// Magic-link handoff: when a paste was created via the JSON API, the response contains a signed
/// `owner` token. Opening `/<id>?owner=<token>` lets the browser claim ownership of the paste, the
/// server validates the token, appends the uid to the user's signed `uid` cookie, and redirects to
/// the clean paste URL so the token never leaks via Referer.
#[derive(Deserialize, Debug)]
pub(crate) struct OwnerHandoff {
    pub(crate) owner: Option<String>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct PasswordForm {
    pub(crate) password: String,
}

#[derive(Deserialize, Debug)]
pub(crate) struct PasteForm {
    #[serde(default)]
    pub(crate) password: Option<String>,
    #[serde(default)]
    pub(crate) confirm_burn: Option<String>,
}

/// Paste view showing the formatted paste.
#[derive(Template, WebTemplate)]
#[template(path = "formatted.html")]
pub(crate) struct Paste {
    page: Page,
    key: Key,
    theme: Option<Theme>,
    lang: Lang,
    can_delete: bool,
    /// If the paste still in the database and can be fetched with another request.
    is_available: bool,
    /// Expiration in case it was set.
    expiration: Option<Expiration>,
    html: String,
    title: Option<String>,
    /// Whether the paste's extension identifies it as Markdown, enabling the rendered-view toggle.
    is_markdown: bool,
}

/// Return `true` if `ext` identifies a Markdown paste.
pub(crate) fn is_markdown_ext(ext: Option<&str>) -> bool {
    ext.is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
}

#[expect(clippy::too_many_arguments)]
pub async fn get<E>(
    State(cache): State<Cache>,
    State(page): State<Page>,
    State(db): State<Database>,
    State(highlighter): State<Highlighter>,
    State(cookie_key): State<CookieKey>,
    Path(id): Path<String>,
    Query(handoff): Query<OwnerHandoff>,
    jar: SignedCookieJar,
    uids: Option<Uids>,
    theme: Option<Theme>,
    lang: Lang,
    form: Result<Form<PasteForm>, E>,
) -> Result<Response, ErrorResponse> {
    if let Some(token) = handoff.owner.as_deref()
        && let Some(claimed_uid) = verify_owner_token(&cookie_key, token)
    {
        let mut new_uids = uids
            .as_ref()
            .map(|Uids(list)| list.clone())
            .unwrap_or_default();
        if !new_uids.contains(&claimed_uid) {
            new_uids.push(claimed_uid);
        }
        let mut cookie = cookie("uid", serialize_uids(&new_uids));
        cookie.set_secure(true);
        return Ok((jar.add(cookie), crate::redirect(&format!("/{id}"))).into_response());
    }

    async {
        let form = form.ok().map(|Form(form)| form);
        let password = form
            .as_ref()
            .and_then(|form| form.password.as_ref())
            .filter(|password| !password.is_empty())
            .map(|password| Password::from(password.as_bytes().to_vec()));
        let confirmed = form.as_ref().and_then(|form| form.confirm_burn.as_deref()) == Some("1");
        let no_password = password.is_none();
        let key: Key = id.parse()?;

        let metadata = match db.get_metadata(key.id).await {
            Ok(metadata) => metadata,
            Err(err) => return Err(err.into()),
        };

        if metadata.must_be_deleted && !confirmed {
            return Ok(BurnConfirmation {
                page: page.clone(),
                theme: theme.clone(),
                lang,
                id,
                title: metadata.title.clone(),
            }
            .into_response());
        }

        let (data, is_available) = match db.get(key.id, password).await {
            Ok(Entry::Regular(data)) => (data, true),
            Ok(Entry::Burned(data)) => (data, false),
            Err(db::Error::NoPassword) => {
                return Ok(PasswordInput {
                    page: page.clone(),
                    theme: theme.clone(),
                    lang,
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
            ..
        } = metadata;

        let can_delete = match (uids, owner_uid) {
            (Some(Uids(uids)), Some(owner_uid)) => uids.contains(&owner_uid),
            _ => false,
        };

        let html = if let Some(html) = cache.get(&key, Mode::Source) {
            tracing::trace!(?key, "found cached item");
            html.into_inner()
        } else {
            let ext = key.ext.clone();
            let highlighter = highlighter.clone();
            let html =
                tokio::task::spawn_blocking(move || highlighter.highlight(text, ext)).await??;

            if is_available && no_password {
                tracing::trace!(?key, "cache item");
                cache.put(&key, Mode::Source, html.clone());
            }

            html.into_inner()
        };

        let is_markdown = is_markdown_ext(key.ext.as_deref());
        let paste = Paste {
            page: page.clone(),
            key,
            theme: theme.clone(),
            lang,
            can_delete,
            is_available,
            expiration,
            html,
            title,
            is_markdown,
        };

        Ok(paste.into_response())
    }
    .await
    .map_err(|err| make_error(err, page, theme, lang))
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
