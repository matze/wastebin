use crate::cache::Key;
use crate::handlers::extract::Theme;
use crate::handlers::html::qr::{code_from, dark_modules};
use crate::handlers::html::{ErrorResponse, make_error};
use crate::{Error, Page};
use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};

/// GET handler for the burn page.
pub async fn get(
    Path(id): Path<String>,
    State(page): State<Page>,
    theme: Option<Theme>,
) -> Result<Burn, ErrorResponse> {
    async {
        let key: Key = id.parse()?;

        let code = tokio::task::spawn_blocking({
            let page = page.clone();
            move || code_from(&page.base_url, &id)
        })
        .await
        .map_err(Error::from)??;

        Ok(Burn {
            page: page.clone(),
            key,
            code,
            theme: theme.clone(),
        })
    }
    .await
    .map_err(|err| make_error(err, page, theme))
}

/// Burn page shown if "burn-after-reading" was selected during insertion.
#[derive(Template, WebTemplate)]
#[template(path = "burn.html", escape = "none")]
pub(crate) struct Burn {
    page: Page,
    key: Key,
    code: qrcodegen::QrCode,
    theme: Option<Theme>,
}

impl Burn {
    fn dark_modules(&self) -> Vec<(i32, i32)> {
        dark_modules(&self.code)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::Client;
    use crate::{handlers::insert::form::Entry, test_helpers::StoreCookies};
    use reqwest::{StatusCode, header};
    use serde::Serialize;

    #[tokio::test]
    async fn burn() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let data = Entry {
            burn_after_reading: Some(String::from("on")),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let location = res.headers().get("location").unwrap().to_str()?;

        // Location is the `/burn/foo` page not the paste itself, so remove the prefix.
        let location = location.replace("burn/", "");

        let res = client
            .get(&location)
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;

        assert_eq!(res.status(), StatusCode::OK);

        let res = client
            .get(&location)
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;

        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }

    #[tokio::test]
    async fn burn_encrypted() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(StoreCookies(false)).await;
        let password = "asd";
        let data = Entry {
            password: password.to_string(),
            burn_after_reading: Some(String::from("on")),
            ..Default::default()
        };

        let res = client.post_form().form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let location = res.headers().get("location").unwrap().to_str()?;

        // Location is the `/burn/foo` page not the paste itself, so remove the prefix.
        let location = location.replace("burn/", "");

        let res = client
            .get(&location)
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;

        assert_eq!(res.status(), StatusCode::OK);

        {
            #[derive(Debug, Serialize)]
            struct Form {
                password: String,
            }

            let data = Form {
                password: password.to_string(),
            };

            let res = client
                .post(&location)
                .form(&data)
                .header(header::ACCEPT, "text/html; charset=utf-8")
                .send()
                .await?;

            assert_eq!(res.status(), StatusCode::OK);
        }

        let res = client
            .get(&location)
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;

        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }
}
