use std::fmt::Write;

use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum_extra::headers::HeaderValue;

use crate::Page;
use crate::cache::Key;
use crate::handlers::extract::{Password, Theme};
use crate::handlers::html::{ErrorResponse, PasswordInput, make_error};
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
            Err(db::Error::NoPassword) => Ok(PasswordInput {
                page: page.clone(),
                theme: theme.clone(),
                lang,
                id: key.id.to_string(),
            }
            .into_response()),
            Err(err) => Err(err.into()),
        }
    }
    .await
    .map_err(|err| make_error(err, page, theme, lang))
}

fn make_content_disposition(filename: &str) -> HeaderValue {
    let mut value = String::from("attachment; filename*=UTF-8''");

    for b in filename.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'~' | b'+') {
            value.push(b as char);
        } else {
            write!(value, "%{b:02X}").expect("writing to String");
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
}
