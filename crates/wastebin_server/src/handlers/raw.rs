use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};

use crate::cache::Key;
use crate::handlers::NoStore;
use crate::handlers::extract::{Password, Theme};
use crate::handlers::html::{ErrorResponse, PasswordInput, make_error};
use crate::i18n::Lang;
use crate::{Database, Page};
use wastebin_core::db;
use wastebin_core::db::read::Entry;

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
        let password = password.map(|Password(password)| password);
        let key: Key = id.parse()?;

        match db.get(key.id, password).await {
            Ok(Entry::Regular(data) | Entry::Burned(data)) => Ok(data.text.into_response()),
            Ok(Entry::Encrypted(data)) => Ok(data.text.into_response().no_store()),
            Err(db::Error::NoPassword) => Ok(PasswordInput {
                page: page.clone(),
                theme: theme.clone(),
                lang,
                id: key.id.to_string(),
                is_rendered: false,
            }
            .into_response()
            .no_store()),
            Err(err) => Err(err.into()),
        }
    }
    .await
    .map_err(|err| make_error(err, page, theme, lang))
}

#[cfg(test)]
mod tests {
    use crate::handlers::extract::PASSWORD_HEADER_NAME;
    use crate::handlers::insert::form::Entry;
    use crate::test_helpers::{Client, StoreCookies};
    use reqwest::StatusCode;
    use reqwest::header;

    #[tokio::test]
    async fn encrypted_raw_is_not_stored() -> Result<(), Box<dyn std::error::Error>> {
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

        // Password prompt for the encrypted paste must not be cached either.
        let res = client.get(&format!("/raw{location}")).send().await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );

        // Decrypted content must not be stored.
        let res = client
            .get(&format!("/raw{location}"))
            .header(PASSWORD_HEADER_NAME, password)
            .send()
            .await?;
        assert_eq!(res.status(), StatusCode::OK);
        let cache_control = res.headers().get(header::CACHE_CONTROL).cloned();
        assert_eq!(res.text().await?, "secret-body-xyz");
        assert_eq!(cache_control.unwrap(), "no-store");

        // Plain pastes stay cacheable.
        let data = Entry {
            text: String::from("plain"),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        let location = res.headers().get("location").unwrap().to_str()?.to_owned();

        let res = client.get(&format!("/raw{location}")).send().await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(res.headers().get(header::CACHE_CONTROL).is_none());

        Ok(())
    }
}
