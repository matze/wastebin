use crate::pages::{Burn, Index};
use crate::AppState;
use axum::extract::Path;
use axum::routing::{get, Router};

mod assets;
mod form;
mod json;
pub(crate) mod paste;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(|| async { Index::default() }).post(paste::insert))
        .route(
            "/:id",
            get(paste::get).post(paste::get).delete(paste::delete),
        )
        .route("/burn/:id", get(|Path(id)| async { Burn::new(id) }))
        .route("/delete/:id", get(paste::delete))
        .merge(assets::routes())
}

#[cfg(test)]
mod tests {
    use crate::db::write::Entry;
    use crate::env::base_path;
    use crate::routes;
    use crate::test_helpers::{make_app, Client};
    use http::StatusCode;
    use reqwest::header;
    use serde::Serialize;

    #[tokio::test]
    async fn unknown_paste() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(make_app()?).await;

        let res = client.get(&base_path().join("000000")).send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }

    #[tokio::test]
    async fn insert_via_form() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(make_app()?).await;

        let data = routes::form::Entry {
            text: "FooBarBaz".to_string(),
            extension: Some("rs".to_string()),
            expires: "0".to_string(),
            password: "".to_string(),
        };

        let res = client.post(base_path().path()).form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let location = res.headers().get("location").unwrap().to_str()?;

        let res = client
            .get(location)
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .send()
            .await?;

        assert_eq!(res.status(), StatusCode::OK);

        let header = res.headers().get(header::CONTENT_TYPE).unwrap();
        assert!(header.to_str().unwrap().contains("text/html"));

        let content = res.text().await?;
        assert!(content.contains("FooBarBaz"));

        let res = client
            .get(location)
            .header(header::ACCEPT, "text/html; charset=utf-8")
            .query(&[("fmt", "raw")])
            .send()
            .await?;

        assert_eq!(res.status(), StatusCode::OK);

        let header = res.headers().get(header::CONTENT_TYPE).unwrap();
        assert!(header.to_str().unwrap().contains("text/plain"));

        let content = res.text().await?;
        assert_eq!(content, "FooBarBaz");

        Ok(())
    }

    #[tokio::test]
    async fn burn_after_reading() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(make_app()?).await;

        let data = routes::form::Entry {
            text: "FooBarBaz".to_string(),
            extension: None,
            expires: "burn".to_string(),
            password: "".to_string(),
        };

        let res = client.post(base_path().path()).form(&data).send().await?;
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
    async fn burn_after_reading_with_encryption() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(make_app()?).await;
        let password = "asd";

        let data = routes::form::Entry {
            text: "FooBarBaz".to_string(),
            extension: None,
            expires: "burn".to_string(),
            password: password.to_string(),
        };

        let res = client.post(base_path().path()).form(&data).send().await?;
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

    #[tokio::test]
    async fn insert_via_json() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(make_app()?).await;

        let entry = Entry {
            text: "FooBarBaz".to_string(),
            ..Default::default()
        };

        let res = client.post(base_path().path()).json(&entry).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let payload = res.json::<routes::json::RedirectResponse>().await?;

        let res = client.get(&payload.path).send().await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await?, "FooBarBaz");

        Ok(())
    }

    #[tokio::test]
    async fn insert_via_json_encrypted() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(make_app()?).await;
        let password = "SuperSecretPassword";

        let entry = Entry {
            text: "FooBarBaz".to_string(),
            password: Some(password.to_string()),
            ..Default::default()
        };

        let res = client.post(base_path().path()).json(&entry).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let payload = res.json::<routes::json::RedirectResponse>().await?;

        let res = client
            .get(&payload.path)
            .header("Wastebin-Password", password)
            .send()
            .await?;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await?, "FooBarBaz");

        Ok(())
    }

    #[tokio::test]
    async fn delete_via_link() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(make_app()?).await;

        let data = routes::form::Entry {
            text: "FooBarBaz".to_string(),
            extension: None,
            expires: "0".to_string(),
            password: "".to_string(),
        };

        let res = client.post(base_path().path()).form(&data).send().await?;
        let uid_cookie = res.cookies().find(|cookie| cookie.name() == "uid");
        assert!(uid_cookie.is_some());
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let location = res.headers().get("location").unwrap().to_str()?;
        let id = location.replace(base_path().path(), "");

        let res = client
            .get(&base_path().join(&format!("delete/{id}")))
            .send()
            .await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let res = client.get(&base_path().join(&id)).send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }

    #[tokio::test]
    async fn download() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(make_app()?).await;

        let data = routes::form::Entry {
            text: "FooBarBaz".to_string(),
            extension: None,
            expires: "0".to_string(),
            password: "".to_string(),
        };

        let res = client.post(base_path().path()).form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let location = res.headers().get("location").unwrap().to_str()?;
        let res = client.get(&format!("{location}?dl=cpp")).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let content = res.text().await?;
        assert_eq!(content, "FooBarBaz");

        Ok(())
    }
}
