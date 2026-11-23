use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Form, Path, State};
use axum::response::{IntoResponse, Response};

use crate::cache::{Key, Mode};
use crate::handlers::NoStore;
use crate::handlers::extract::{Theme, Uids};
use crate::handlers::html::paste::PasteForm;
use crate::handlers::html::{ErrorResponse, FormAction, Unlock, Unlocked, make_error, unlock};
use crate::i18n::Lang;
use crate::{Cache, Database, Highlighter, Page};
use wastebin_core::db::read::{Data, Metadata};
use wastebin_core::expiration::Expiration;
use wastebin_highlight::markdown;

/// Page showing a Markdown paste rendered as HTML.
#[derive(Template, WebTemplate)]
#[template(path = "rendered.html")]
pub(crate) struct Rendered {
    page: Page,
    key: Key,
    theme: Option<Theme>,
    lang: Lang,
    can_delete: bool,
    is_available: bool,
    /// Always `true` for this view; needed by the inherited paste template.
    is_markdown: bool,
    /// Always `false` for this view; needed by the inherited paste template.
    is_html: bool,
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
    uids: Option<Uids>,
    theme: Option<Theme>,
    lang: Lang,
    form: Result<Form<PasteForm>, E>,
) -> Result<Response, ErrorResponse> {
    async {
        let key: Key = id.parse()?;
        let form = form.ok().map(|Form(form)| form);
        let action = FormAction::rendered(&key);
        let Unlocked {
            data,
            is_available,
            encrypted,
            no_password,
        } = match unlock(&db, &page, theme.as_ref(), lang, &key, form, action).await? {
            Unlock::Paste(unlocked) => unlocked,
            Unlock::Prompt(response) => return Ok(response),
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

        let html = if let Some(cached) = cache.get(&key, Mode::Rendered) {
            tracing::trace!(?key, "found cached rendered markdown");
            cached.into_inner()
        } else {
            let highlighter = highlighter.clone();
            let rendered =
                tokio::task::spawn_blocking(move || markdown::render(&text, &highlighter))
                    .await??;

            if is_available && no_password {
                tracing::trace!(?key, "cache rendered markdown");
                cache.put(&key, Mode::Rendered, rendered.clone());
            }

            rendered.into_inner()
        };

        let rendered = Rendered {
            page: page.clone(),
            key,
            theme: theme.clone(),
            lang,
            can_delete,
            is_available,
            is_markdown: true,
            is_html: false,
            expiration,
            html,
            title,
        };

        let response = rendered.into_response();
        Ok(if encrypted {
            response.no_store()
        } else {
            response
        })
    }
    .await
    .map_err(|err| make_error(err, page, theme, lang))
}

#[cfg(test)]
mod tests {
    use crate::handlers::insert::form::Entry;
    use crate::test_helpers::{Client, StoreCookies};
    use reqwest::{StatusCode, header};

    #[tokio::test]
    async fn renders_markdown_as_html() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let data = Entry {
            text: String::from("# Hello\n\n| a | b |\n|---|---|\n| 1 | 2 |\n"),
            extension: Some(String::from("md")),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let location = res.headers().get("location").unwrap().to_str()?.to_owned();

        let res = client
            .get(&format!("/md{location}"))
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;

        assert_eq!(res.status(), StatusCode::OK);

        let body = res.text().await?;
        assert!(body.contains("markdown-body"), "body: {body}");
        assert!(body.contains("<h1>Hello</h1>"), "body: {body}");
        assert!(body.contains("<th>a</th>"), "body: {body}");

        Ok(())
    }

    #[tokio::test]
    async fn burn_requires_confirmation() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let data = Entry {
            text: String::from("# secret\n"),
            extension: Some(String::from("md")),
            burn_after_reading: Some(String::from("on")),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        // Location is the `/burn/foo` page not the paste itself, so remove the prefix.
        let location = res
            .headers()
            .get("location")
            .unwrap()
            .to_str()?
            .replace("burn/", "");
        let rendered = format!("/md{location}");

        // First GET of the rendered view shows the confirmation interstitial without revealing
        // content, and confirms against the rendered route.
        let res = client
            .get(&rendered)
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.text().await?;
        assert!(body.contains("confirm_burn"), "body: {body}");
        assert!(
            body.contains(&format!("action=\"{rendered}\"")),
            "body: {body}"
        );
        assert!(!body.contains("<h1>secret</h1>"), "body: {body}");

        // Second GET must still show the confirmation — the paste is not yet burned.
        let res = client
            .get(&rendered)
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(res.text().await?.contains(">reveal<"));

        // Confirming reveals the rendered paste and burns it.
        let res = client
            .post(&rendered)
            .form(&[("confirm_burn", "1")])
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(res.text().await?.contains("<h1>secret</h1>"));

        // Subsequent GETs 404 — the paste was burned.
        let res = client
            .get(&rendered)
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }

    #[tokio::test]
    async fn missing_paste_is_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;

        let res = client.get("/md/000000").send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }

    #[tokio::test]
    async fn rendered_response_relaxes_img_src() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let data = Entry {
            text: String::from("# picture\n\n![cat](https://example.com/cat.png)\n"),
            extension: Some(String::from("md")),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        let location = res.headers().get("location").unwrap().to_str()?.to_owned();

        let rendered = client.get(&format!("/md{location}")).send().await?;
        let csp = rendered
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()?
            .to_owned();
        assert!(csp.contains("img-src * data:"), "csp: {csp}");

        let source = client.get(&location).send().await?;
        let csp = source
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()?;
        assert!(csp.contains("img-src 'self' data:"), "csp: {csp}");

        Ok(())
    }

    #[tokio::test]
    async fn encrypted_rendered_is_not_stored() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let password = "hunter2";
        let data = Entry {
            text: String::from("# secret\n"),
            extension: Some(String::from("md")),
            password: password.to_string(),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let location = res.headers().get("location").unwrap().to_str()?.to_owned();

        // The password prompt is not cacheable.
        let res = client.get(&format!("/md{location}")).send().await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );

        // The decrypted Markdown is not cacheable either.
        let res = client
            .post(&format!("/md{location}"))
            .form(&[("password", password)])
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;
        assert_eq!(res.status(), StatusCode::OK);
        let cache_control = res.headers().get(header::CACHE_CONTROL).cloned();
        let body = res.text().await?;
        assert!(body.contains("<h1>secret</h1>"), "body: {body}");
        assert_eq!(cache_control.unwrap(), "no-store");

        Ok(())
    }
}
