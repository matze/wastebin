use crate::cache::Key;
use crate::db::read::Entry;
use crate::handlers::extract::Password;
use crate::handlers::html::{make_error, ErrorResponse, PasswordInput};
use crate::{Database, Error, Page};
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{AppendHeaders, IntoResponse, Response};
use axum_extra::headers::HeaderValue;

/// GET handler for raw content of a paste.
pub async fn download(
    Path(id): Path<String>,
    State(db): State<Database>,
    State(page): State<Page>,
    password: Option<Password>,
) -> Result<Response, ErrorResponse> {
    async {
        let key: Key = id.parse()?;
        let password = password.map(|Password(password)| password);

        match db.get(key.id, password).await {
            Ok(Entry::Regular(data) | Entry::Burned(data)) => {
                Ok(get_download(data.text, &key.id(), &key.ext).into_response())
            }
            Ok(Entry::Expired) => Err(Error::NotFound),
            Err(Error::NoPassword) => Ok(PasswordInput {
                page: page.clone(),
                id: key.id.to_string(),
            }
            .into_response()),
            Err(err) => Err(err),
        }
    }
    .await
    .map_err(|err| make_error(err, page))
}

fn get_download(text: String, id: &str, extension: &str) -> impl IntoResponse {
    let content_type = "text; charset=utf-8";
    let content_disposition =
        HeaderValue::from_str(&format!(r#"attachment; filename="{id}.{extension}"#))
            .expect("constructing valid header value");

    (
        AppendHeaders([
            (header::CONTENT_TYPE, HeaderValue::from_static(content_type)),
            (header::CONTENT_DISPOSITION, content_disposition),
        ]),
        text,
    )
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::Client;
    use reqwest::StatusCode;

    #[tokio::test]
    async fn download() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new().await;

        let data = crate::handlers::insert::form::Entry {
            text: "FooBarBaz".to_string(),
            extension: None,
            expires: Some("0".to_string()),
            password: "".to_string(),
            title: "".to_string(),
            burn_after_reading: None,
        };

        let res = client.post("/").form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let location = res.headers().get("location").unwrap().to_str()?;
        let res = client.get(&format!("/dl{location}.cpp")).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let content = res.text().await?;
        assert_eq!(content, "FooBarBaz");

        Ok(())
    }
}
