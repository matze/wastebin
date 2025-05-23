use crate::Page;
use crate::cache::Key;
use crate::handlers::extract::{Password, Theme};
use crate::handlers::html::{ErrorResponse, PasswordInput, make_error};
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum_extra::headers::HeaderValue;
use wastebin_core::db::read::{Data, Entry};
use wastebin_core::db::{self, Database};

/// GET handler for raw content of a paste.
pub async fn get(
    Path(id): Path<String>,
    State(db): State<Database>,
    State(page): State<Page>,
    theme: Option<Theme>,
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
                id: key.id.to_string(),
            }
            .into_response()),
            Err(err) => Err(err.into()),
        }
    }
    .await
    .map_err(|err| make_error(err, page, theme))
}

fn get_download(key: &Key, data: Data) -> impl IntoResponse {
    let filename = data.metadata.title.unwrap_or_else(|| format!("{key}"));

    let content_type = "text; charset=utf-8";
    let content_disposition =
        HeaderValue::from_str(&format!(r#"attachment; filename="{filename}""#))
            .expect("constructing valid header value");

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
            format!(r#"attachment; filename="{filename}.cpp""#),
        );

        let content = res.text().await?;
        assert_eq!(content, "FooBarBaz");

        let res = client.get(&format!("/dl{location}")).send().await?;
        let content_disposition = res.headers().get(header::CONTENT_DISPOSITION).unwrap();
        assert_eq!(
            content_disposition.to_str()?,
            format!(r#"attachment; filename="{filename}""#),
        );

        Ok(())
    }
}
