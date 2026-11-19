use axum::extract::{Form, Path, State};
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};

use crate::cache::Key;
use crate::handlers::NoStore;
use crate::handlers::extract::Theme;
use crate::handlers::html::paste::PasteForm;
use crate::handlers::html::{
    BurnConfirmation, ErrorResponse, FormAction, PasswordInput, make_error,
};
use crate::i18n::Lang;
use crate::{Database, Page};
use wastebin_core::crypto::Password;
use wastebin_core::db;
use wastebin_core::db::read::Entry;

const CSP_SANDBOX: HeaderValue = HeaderValue::from_static(
    "sandbox; default-src 'none'; style-src 'unsafe-inline' https://fonts.googleapis.com ; font-src https://fonts.gstatic.com data: ; img-src * data: ; base-uri 'none' ; form-action 'none' ; frame-ancestors 'none' ;",
);

pub async fn get<E>(
    State(page): State<Page>,
    State(db): State<Database>,
    Path(id): Path<String>,
    theme: Option<Theme>,
    lang: Lang,
    form: Result<Form<PasteForm>, E>,
) -> Result<Response, ErrorResponse> {
    async {
        if !page.render_html {
            return Err(db::Error::NotFound.into());
        }

        let form = form.ok().map(|Form(form)| form);
        let password = form
            .as_ref()
            .and_then(|form| form.password.as_ref())
            .filter(|password| !password.is_empty())
            .map(|password| Password::from(password.as_bytes().to_vec()));
        let confirmed = form.as_ref().and_then(|form| form.confirm_burn.as_deref()) == Some("1");
        let key: Key = id.parse()?;
        let metadata = db.get_metadata(key.id).await?;

        if metadata.must_be_deleted && !confirmed {
            return Ok(BurnConfirmation {
                page: page.clone(),
                theme: theme.clone(),
                lang,
                title: metadata.title.clone(),
                action: FormAction::html(&key),
            }
            .into_response());
        }

        let (text, encrypted) = match db.get(key.id, password).await {
            Ok(Entry::Regular(data) | Entry::Burned(data)) => (data.text, false),
            Ok(Entry::Encrypted(data)) => (data.text, true),
            Err(db::Error::NoPassword) => {
                return Ok(PasswordInput {
                    page: page.clone(),
                    theme: theme.clone(),
                    lang,
                    action: FormAction::html(&key),
                }
                .into_response()
                .no_store());
            }
            Err(err) => return Err(err.into()),
        };

        let response = (
            [
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/html; charset=utf-8"),
                ),
                (header::CONTENT_SECURITY_POLICY, CSP_SANDBOX),
            ],
            text,
        )
            .into_response();

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
    use crate::test_helpers::{Client, RenderHtml, StoreCookies};
    use reqwest::{StatusCode, header};

    const PAGE: &str = "<!doctype html><title>Spec</title><style>h1{color:teal}</style><h1>Hello</h1><svg viewBox=\"0 0 1 1\"><rect width=\"1\" height=\"1\"/></svg>";

    fn csp(res: &reqwest::Response) -> String {
        res.headers()
            .get(header::CONTENT_SECURITY_POLICY)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned()
    }

    fn assert_sandboxed(csp: &str) {
        assert!(csp.starts_with("sandbox;"), "csp: {csp}");
        assert!(!csp.contains("allow-"), "csp: {csp}");
        assert!(csp.contains("default-src 'none'"), "csp: {csp}");
    }

    fn assert_strict(csp: &str) {
        assert!(!csp.contains("sandbox"), "csp: {csp}");
        assert!(csp.contains("script-src 'self'"), "csp: {csp}");
    }

    async fn insert(client: &Client, data: &Entry) -> Result<String, Box<dyn std::error::Error>> {
        let res = client.post_form().form(data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let location = res.headers().get("location").unwrap().to_str()?;
        Ok(location.replace("burn/", ""))
    }

    #[tokio::test]
    async fn serves_paste_unmodified_in_sandbox() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::with_html(StoreCookies(false), RenderHtml(true)).await;
        let data = Entry {
            text: String::from(PAGE),
            extension: Some(String::from("html")),
            ..Default::default()
        };
        let location = insert(&client, &data).await?;

        let res = client.get(&format!("/html{location}")).send().await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        assert_sandboxed(&csp(&res));
        assert_eq!(res.text().await?, PAGE);

        Ok(())
    }

    #[tokio::test]
    async fn burn_requires_confirmation() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::with_html(StoreCookies(false), RenderHtml(true)).await;
        let data = Entry {
            text: String::from(PAGE),
            extension: Some(String::from("html")),
            burn_after_reading: Some(String::from("on")),
            ..Default::default()
        };
        let location = insert(&client, &data).await?;
        let document = format!("/html{location}");

        let res = client.get(&document).send().await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert_strict(&csp(&res));
        let body = res.text().await?;
        assert!(
            body.contains(&format!("action=\"{document}\"")),
            "body: {body}"
        );
        assert!(!body.contains("<h1>Hello</h1>"), "body: {body}");

        let res = client.get(&document).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let res = client
            .post(&document)
            .form(&[("confirm_burn", "1")])
            .send()
            .await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert_sandboxed(&csp(&res));
        assert_eq!(res.text().await?, PAGE);

        let res = client.get(&document).send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }

    #[tokio::test]
    async fn encrypted_paste_needs_password_and_is_not_stored()
    -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::with_html(StoreCookies(false), RenderHtml(true)).await;
        let password = "hunter2";
        let data = Entry {
            text: String::from(PAGE),
            extension: Some(String::from("html")),
            password: password.to_string(),
            ..Default::default()
        };
        let location = insert(&client, &data).await?;
        let document = format!("/html{location}");

        let res = client.get(&document).send().await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        assert_strict(&csp(&res));
        let body = res.text().await?;
        assert!(
            body.contains(&format!("action=\"{document}\"")),
            "body: {body}"
        );

        let res = client
            .post(&document)
            .form(&[("password", password)])
            .send()
            .await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        assert_sandboxed(&csp(&res));
        assert_eq!(res.text().await?, PAGE);

        Ok(())
    }

    #[tokio::test]
    async fn missing_paste_is_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::with_html(StoreCookies(false), RenderHtml(true)).await;

        let res = client.get("/html/000000").send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        assert_strict(&csp(&res));

        Ok(())
    }

    #[tokio::test]
    async fn source_view_links_to_document() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::with_html(StoreCookies(false), RenderHtml(true)).await;
        let data = Entry {
            text: String::from(PAGE),
            extension: Some(String::from("html")),
            ..Default::default()
        };
        let location = insert(&client, &data).await?;

        let body = client.get(&location).send().await?.text().await?;
        assert!(
            body.contains(&format!("href=\"/html{location}\"")),
            "body: {body}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn disabled_by_default() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let data = Entry {
            text: String::from(PAGE),
            extension: Some(String::from("html")),
            ..Default::default()
        };
        let location = insert(&client, &data).await?;

        let res = client.get(&format!("/html{location}")).send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        assert_strict(&csp(&res));
        assert!(!res.text().await?.contains("<h1>Hello</h1>"));

        let body = client.get(&location).send().await?.text().await?;
        assert!(!body.contains("href=\"/html/"), "body: {body}");

        Ok(())
    }
}
