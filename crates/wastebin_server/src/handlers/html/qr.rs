use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};
use qrcodegen::QrCode;
use url::Url;

use crate::cache::Key;
use crate::handlers::extract::{Theme, Uids};
use crate::handlers::html::paste::is_markdown_ext;
use crate::handlers::html::{ErrorResponse, make_error};
use crate::i18n::Lang;
use crate::{Error, Page};
use wastebin_core::db::Database;
use wastebin_core::db::read::Metadata;
use wastebin_core::expiration::Expiration;

/// GET handler for a QR page.
pub async fn get(
    Path(id): Path<String>,
    State(page): State<Page>,
    State(db): State<Database>,
    uids: Option<Uids>,
    theme: Option<Theme>,
    lang: Lang,
) -> Result<Qr, ErrorResponse> {
    async {
        let key: Key = id.parse()?;

        let code = {
            let page = page.clone();

            tokio::task::spawn_blocking(move || code_from(&page.base_url, &id))
                .await
                .map_err(Error::from)??
        };

        let Metadata {
            uid: owner_uid,
            title,
            expiration,
            ..
        } = db.get_metadata(key.id).await?;

        let can_delete = match (uids, owner_uid) {
            (Some(Uids(uids)), Some(owner_uid)) => uids.contains(&owner_uid),
            _ => false,
        };

        let is_markdown = is_markdown_ext(key.ext.as_deref());

        Ok(Qr {
            page: page.clone(),
            theme: theme.clone(),
            lang,
            key,
            can_delete,
            is_available: true,
            code,
            title,
            expiration,
            is_markdown,
        })
    }
    .await
    .map_err(|err| make_error(err, page, theme, lang))
}

/// Paste view showing the formatted paste as well as a bunch of links.
#[derive(Template, WebTemplate)]
#[template(path = "qr.html")]
pub(crate) struct Qr {
    page: Page,
    theme: Option<Theme>,
    lang: Lang,
    key: Key,
    can_delete: bool,
    is_available: bool,
    is_markdown: bool,
    code: qrcodegen::QrCode,
    title: Option<String>,
    expiration: Option<Expiration>,
}

impl Qr {
    fn dark_modules(&self) -> Vec<(i32, i32)> {
        dark_modules(&self.code)
    }
}

pub fn code_from(url: &Url, id: &str) -> Result<QrCode, Error> {
    Ok(QrCode::encode_text(
        url.join(id)?.as_str(),
        qrcodegen::QrCodeEcc::High,
    )?)
}

/// Return module coordinates that are dark.
pub fn dark_modules(code: &QrCode) -> Vec<(i32, i32)> {
    let size = code.size();
    (0..size)
        .flat_map(|x| (0..size).map(move |y| (x, y)))
        .filter(|(x, y)| code.get_module(*x, *y))
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::handlers::insert::form::Entry;
    use crate::test_helpers::{Client, StoreCookies};
    use reqwest::{StatusCode, header};

    #[tokio::test]
    async fn title_is_escaped() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let payload = "</title><img src=x onerror=alert(1)>";

        let res = client
            .post_form()
            .form(&Entry {
                text: String::from("hello"),
                title: String::from(payload),
                ..Default::default()
            })
            .send()
            .await?;

        let location = res.headers().get("location").unwrap().to_str()?.to_owned();
        let id = location.trim_start_matches('/');

        let res = client
            .get(&format!("/qr/{id}"))
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;

        assert_eq!(res.status(), StatusCode::OK);

        let body = res.text().await?;
        assert!(!body.contains(payload), "title rendered unescaped");
        assert!(!body.contains("<img"), "title produced a live element");
        assert!(
            body.contains("img src=x onerror=alert(1)"),
            "title should still be displayed, just inertly"
        );

        Ok(())
    }

    #[tokio::test]
    async fn extension_is_escaped() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;

        let res = client
            .post_form()
            .form(&Entry {
                text: String::from("hello"),
                ..Default::default()
            })
            .send()
            .await?;

        let location = res.headers().get("location").unwrap().to_str()?.to_owned();
        let id = location.trim_start_matches('/');

        let res = client
            .get(&format!("/qr/{id}.\"><img src=x onerror=alert(1)>"))
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;

        let body = res.text().await?;
        assert!(!body.contains("<img src=x"), "extension rendered unescaped");

        Ok(())
    }
}
