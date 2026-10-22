use std::fmt::Write;

use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum_extra::headers::HeaderValue;

use crate::Page;
use crate::cache::Key;
use crate::handlers::NoStore;
use crate::handlers::extract::{Password, Theme};
use crate::handlers::html::{ErrorResponse, FormAction, PasswordInput, make_error};
use crate::i18n::Lang;
use wastebin_core::db::read::{Data, Entry};
use wastebin_core::db::{self, Database};

/// GET handler for raw content of a paste.
pub async fn get(
    Path(id): Path<String>,
    State(db): State<Database>,
    State(page): State<Page>,
    theme: Option<Theme>,
    lang: Lang,
    password: Option<Password>,
) -> Result<Response, ErrorResponse> {
    async {
        let key: Key = id.parse()?;
        let password = password.map(|Password(password)| password);

        match db.get(key.id, password).await {
            Ok(Entry::Regular(data) | Entry::Burned(data)) => {
                Ok(get_download(&key, data).into_response())
            }
            Ok(Entry::Encrypted(data)) => Ok(get_download(&key, data).into_response().no_store()),
            Err(db::Error::NoPassword) => Ok(PasswordInput {
                page: page.clone(),
                theme: theme.clone(),
                lang,
                action: FormAction::source(&key),
            }
            .into_response()
            .no_store()),
            Err(err) => Err(err.into()),
        }
    }
    .await
    .map_err(|err| make_error(err, page, theme, lang))
}

/// Return `true` if `c` reorders the text around it rather than showing a glyph of its own.
///
/// Percent-encoding carries these through `filename*` intact, and the client decodes them back
/// before showing the name, so `evil<U+202E>gnp.exe` is offered as `evil.exe.png` — the extension
/// the user reads is not the one the file has.
fn reorders_text(c: char) -> bool {
    matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

fn make_content_disposition(filename: &str) -> HeaderValue {
    let mut value = String::from("attachment; filename*=UTF-8''");
    let mut encoded = [0u8; 4];

    // The title is attacker-controlled, so the characters that would misrepresent the name are
    // replaced before it is encoded.
    for c in filename.chars() {
        let c = if c.is_control() || reorders_text(c) {
            '_'
        } else {
            c
        };

        for &b in c.encode_utf8(&mut encoded).as_bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'~' | b'+') {
                value.push(b as char);
            } else {
                write!(value, "%{b:02X}").expect("writing to String");
            }
        }
    }

    HeaderValue::try_from(value).unwrap_or_else(|_| HeaderValue::from_static("attachment"))
}

fn get_download(key: &Key, data: Data) -> impl IntoResponse {
    let filename = data.metadata.title.unwrap_or_else(|| key.to_string());

    let content_type = "text; charset=utf-8";
    let content_disposition = make_content_disposition(&filename);

    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(content_type)),
            (header::CONTENT_DISPOSITION, content_disposition),
        ],
        data.text,
    )
}

#[cfg(test)]
mod tests {
    use crate::handlers::insert::form::Entry;
    use crate::test_helpers::{Client, StoreCookies};
    use http::header;
    use reqwest::StatusCode;

    #[tokio::test]
    async fn download() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let data = Entry {
            text: String::from("FooBarBaz"),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let location = res.headers().get("location").unwrap().to_str()?;
        let filename = &location[1..];
        let res = client.get(&format!("/dl/{filename}.cpp")).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let content_disposition = res.headers().get(header::CONTENT_DISPOSITION).unwrap();
        assert_eq!(
            content_disposition.to_str()?,
            format!("attachment; filename*=UTF-8''{filename}.cpp"),
        );

        let content = res.text().await?;
        assert_eq!(content, "FooBarBaz");

        let res = client.get(&format!("/dl{location}")).send().await?;
        let content_disposition = res.headers().get(header::CONTENT_DISPOSITION).unwrap();
        assert_eq!(
            content_disposition.to_str()?,
            format!("attachment; filename*=UTF-8''{filename}"),
        );

        Ok(())
    }

    #[tokio::test]
    async fn download_title_with_quotes() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let data = Entry {
            text: String::from("content"),
            title: String::from(r#"file"name.txt"#),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        let location = res.headers().get("location").unwrap().to_str()?;
        let res = client.get(&format!("/dl{location}")).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let content_disposition = res.headers().get(header::CONTENT_DISPOSITION).unwrap();
        assert_eq!(
            content_disposition.to_str()?,
            "attachment; filename*=UTF-8''file%22name.txt",
        );

        Ok(())
    }

    /// Percent-encoding is transport, not sanitisation: the client decodes `filename*` back before
    /// showing the name, so a right-to-left override survived into it and offered
    /// `evil<U+202E>gnp.exe` as `evil.exe.png`.
    #[tokio::test]
    async fn download_title_reordering_the_extension() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let data = Entry {
            text: String::from("content"),
            title: String::from("evil\u{202e}gnp\u{7}.exe"),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        let location = res.headers().get("location").unwrap().to_str()?;
        let res = client.get(&format!("/dl{location}")).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let content_disposition = res.headers().get(header::CONTENT_DISPOSITION).unwrap();
        assert_eq!(
            content_disposition.to_str()?,
            "attachment; filename*=UTF-8''evil_gnp_.exe",
        );

        Ok(())
    }

    #[tokio::test]
    async fn download_title_with_non_ascii() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let data = Entry {
            text: String::from("content"),
            title: String::from("café.txt"),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        let location = res.headers().get("location").unwrap().to_str()?;
        let res = client.get(&format!("/dl{location}")).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let content_disposition = res.headers().get(header::CONTENT_DISPOSITION).unwrap();
        assert_eq!(
            content_disposition.to_str()?,
            "attachment; filename*=UTF-8''caf%C3%A9.txt",
        );

        Ok(())
    }

    #[tokio::test]
    async fn encrypted_download_is_not_stored() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let password = "hunter2";
        let data = Entry {
            text: String::from("secret-body-xyz"),
            password: password.to_string(),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let location = res.headers().get("location").unwrap().to_str()?.to_owned();

        let res = client
            .get(&format!("/dl{location}"))
            .header(crate::handlers::extract::PASSWORD_HEADER_NAME, password)
            .send()
            .await?;
        assert_eq!(res.status(), StatusCode::OK);
        let cache_control = res.headers().get(header::CACHE_CONTROL).cloned();
        assert_eq!(res.text().await?, "secret-body-xyz");
        assert_eq!(cache_control.unwrap(), "no-store");

        Ok(())
    }
}
