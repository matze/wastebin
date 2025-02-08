use crate::cache::Key;
use crate::crypto::Password;
use crate::handlers::html::{make_error, ErrorResponse, PasswordInput};
use crate::{Database, Error, Page};
use axum::extract::{Form, Path, State};
use axum::http::header;
use axum::response::{AppendHeaders, IntoResponse, Response};
use axum_extra::headers::HeaderValue;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct PasswordForm {
    password: String,
}

/// GET handler for raw content of a paste.
pub async fn download(
    Path(id): Path<String>,
    State(db): State<Database>,
    State(page): State<Page>,
    form: Option<Form<PasswordForm>>,
) -> Result<Response, ErrorResponse> {
    async {
        let password = form.map(|form| Password::from(form.password.as_bytes().to_vec()));
        let key: Key = id.parse()?;

        match db.get(key.id, password.clone()).await {
            Err(Error::NoPassword) => Ok(PasswordInput {
                page: page.clone(),
                id: key.id.to_string(),
            }
            .into_response()),
            Err(err) => Err(err),
            Ok(entry) => {
                if entry.must_be_deleted {
                    db.delete(key.id).await?;
                }

                Ok(get_download(entry.text, &key.id(), &key.ext).into_response())
            }
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
            expires: "0".to_string(),
            password: "".to_string(),
            title: "".to_string(),
        };

        let res = client.post("/").form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let location = res.headers().get("location").unwrap().to_str()?;
        let res = client.get(&format!("{location}?dl=cpp")).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let content = res.text().await?;
        assert_eq!(content, "FooBarBaz");

        Ok(())
    }
}
