use crate::cache::Key;
use crate::handlers::html::qr::{code_from, dark_modules};
use crate::handlers::html::{make_error, ErrorResponse};
use crate::{Error, Page};
use askama::Template;
use axum::extract::{Path, State};

/// GET handler for the burn page.
pub async fn burn(Path(id): Path<String>, State(page): State<Page>) -> Result<Burn, ErrorResponse> {
    async {
        let code = tokio::task::spawn_blocking({
            let page = page.clone();
            let id = id.clone();
            move || code_from(&page.base_url, &id)
        })
        .await
        .map_err(Error::from)??;

        let key: Key = id.parse()?;

        Ok(Burn {
            page: page.clone(),
            key,
            code,
        })
    }
    .await
    .map_err(|err| make_error(err, page))
}

/// Burn page shown if "burn-after-reading" was selected during insertion.
#[derive(Template)]
#[template(path = "burn.html", escape = "none")]
pub struct Burn {
    page: Page,
    key: Key,
    code: qrcodegen::QrCode,
}

impl Burn {
    fn dark_modules(&self) -> Vec<(i32, i32)> {
        dark_modules(&self.code)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::Client;
    use reqwest::{header, StatusCode};
    use serde::Serialize;

    #[tokio::test]
    async fn burn() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new().await;

        let data = crate::handlers::insert::form::Entry {
            text: "FooBarBaz".to_string(),
            extension: None,
            expires: "burn".to_string(),
            password: "".to_string(),
            title: "".to_string(),
        };

        let res = client.post("/").form(&data).send().await?;
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
        let client = Client::new().await;
        let password = "asd";

        let data = crate::handlers::insert::form::Entry {
            text: "FooBarBaz".to_string(),
            extension: None,
            expires: "burn".to_string(),
            password: password.to_string(),
            title: "".to_string(),
        };

        let res = client.post("/").form(&data).send().await?;
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

        let res = client
            .get(&location)
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;

        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }
}
