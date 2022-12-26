use crate::cache::{Key, Layer};
use crate::db::Entry;
use crate::id::Id;
use crate::Router;
use crate::{highlight, pages, Error};
use askama_axum::IntoResponse;
use axum::body::Body;
use axum::extract::{Form, Path, Query, State};
use axum::headers::{HeaderMapExt, HeaderValue};
use axum::http::header::{self, HeaderMap};
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponseParts, Redirect, Response};
use axum::routing::get;
use axum::{headers, Json, RequestExt, TypedHeader};
use bytes::Bytes;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
struct FormEntry {
    text: String,
    extension: Option<String>,
    expires: String,
}

impl From<FormEntry> for Entry {
    fn from(entry: FormEntry) -> Self {
        let burn_after_reading = Some(entry.expires == "burn");

        let expires = match entry.expires.parse::<u32>() {
            Ok(0) | Err(_) => None,
            Ok(secs) => Some(secs),
        };

        Self {
            text: entry.text,
            extension: entry.extension,
            expires,
            burn_after_reading,
            seconds_since_creation: 0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonEntry {
    text: String,
    extension: Option<String>,
    expires: Option<u32>,
    burn_after_reading: Option<bool>,
}

impl From<JsonEntry> for Entry {
    fn from(entry: JsonEntry) -> Self {
        Self {
            text: entry.text,
            extension: entry.extension,
            expires: entry.expires,
            burn_after_reading: entry.burn_after_reading,
            seconds_since_creation: 0,
        }
    }
}

#[derive(Serialize)]
struct ErrorPayload {
    message: String,
}

type ErrorResponse = (StatusCode, Json<ErrorPayload>);

impl From<Error> for ErrorResponse {
    fn from(err: Error) -> Self {
        let payload = Json::from(ErrorPayload {
            message: err.to_string(),
        });

        (err.into(), payload)
    }
}

fn index<'a>() -> pages::Index<'a> {
    pages::Index::default()
}

async fn insert_from_form(
    Form(entry): Form<FormEntry>,
    layer: State<Layer>,
) -> Result<Redirect, pages::ErrorResponse<'static>> {
    let id: Id = tokio::task::spawn_blocking(|| {
        let mut rng = rand::thread_rng();
        rng.gen::<u32>()
    })
    .await
    .map_err(Error::from)?
    .into();

    let entry: Entry = entry.into();
    let url = id.to_url_path(&entry);
    let burn_after_reading = entry.burn_after_reading.unwrap_or(false);

    layer.insert(id, entry).await?;

    if burn_after_reading {
        Ok(Redirect::to(&format!("/burn{url}")))
    } else {
        Ok(Redirect::to(&url))
    }
}

#[derive(Deserialize, Serialize)]
struct RedirectResponse {
    path: String,
}

async fn insert_from_json(
    Json(entry): Json<JsonEntry>,
    layer: State<Layer>,
) -> Result<Json<RedirectResponse>, ErrorResponse> {
    let id: Id = tokio::task::spawn_blocking(|| {
        let mut rng = rand::thread_rng();
        rng.gen::<u32>()
    })
    .await
    .map_err(Error::from)?
    .into();

    let entry: Entry = entry.into();
    let path = id.to_url_path(&entry);

    layer.insert(id, entry).await?;

    Ok(Json::from(RedirectResponse { path }))
}

async fn insert(
    layer: State<Layer>,
    headers: HeaderMap,
    request: Request<Body>,
) -> Result<Response, Response> {
    let content_type = headers
        .typed_get::<headers::ContentType>()
        .ok_or_else(|| StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response())?;

    if content_type == headers::ContentType::form_url_encoded() {
        let entry: Form<FormEntry> = request
            .extract()
            .await
            .map_err(IntoResponse::into_response)?;

        Ok(insert_from_form(entry, layer).await.into_response())
    } else if content_type == headers::ContentType::json() {
        let entry: Json<JsonEntry> = request
            .extract()
            .await
            .map_err(IntoResponse::into_response)?;

        Ok(insert_from_json(entry, layer).await.into_response())
    } else {
        Err(StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response())
    }
}

async fn get_html(
    id: Path<String>,
    layer: Layer,
) -> Result<pages::Paste<'static>, pages::ErrorResponse<'static>> {
    let key = Key::try_from(id)?;
    let entry = layer.get_formatted(&key).await?;

    Ok(pages::Paste::new(entry, &key))
}

async fn get_raw(id: Path<String>, layer: Layer) -> Result<String, ErrorResponse> {
    let key = Key::try_from(id)?;

    Ok(layer.get(Id::try_from(key.id().as_str())?).await?.text)
}

async fn get_download(
    Path(id): Path<String>,
    extension: String,
    layer: Layer,
) -> Result<Response<String>, pages::ErrorResponse<'static>> {
    // Validate extension.
    if !extension.is_ascii() {
        Err(Error::IllegalCharacters)?;
    }

    let raw_string = layer.get(Id::try_from(id.as_str())?).await?.text;
    let content_type = "text; charset=utf-8";
    let content_disposition = format!(r#"attachment; filename="{id}.{extension}"#);

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
        .header(header::CONTENT_DISPOSITION, content_disposition)
        .body(raw_string)
        .map_err(Error::from)?)
}

#[derive(Deserialize, Debug)]
struct GetQuery {
    fmt: Option<String>,
    dl: Option<String>,
}

async fn get_paste(
    id: Path<String>,
    headers: HeaderMap,
    Query(query): Query<GetQuery>,
    State(layer): State<Layer>,
) -> Response {
    if let Some(fmt) = query.fmt {
        if fmt == "raw" {
            return get_raw(id, layer).await.into_response();
        }
    }

    if let Some(extension) = query.dl {
        return get_download(id, extension, layer).await.into_response();
    }

    if let Some(value) = headers.get(header::ACCEPT) {
        if let Ok(value) = value.to_str() {
            if value.contains("text/html") {
                return get_html(id, layer).await.into_response();
            }
        }
    }

    get_raw(id, layer).await.into_response()
}

async fn delete(
    Path(id): Path<String>,
    layer: State<Layer>,
) -> Result<Redirect, pages::ErrorResponse<'static>> {
    let id = Id::try_from(id.as_str())?;
    let entry = layer.get(id).await?;

    if entry.seconds_since_creation > 60 {
        Err(Error::DeletionTimeExpired)?;
    }

    layer.delete(id).await?;

    Ok(Redirect::to("/"))
}

fn css_headers() -> impl IntoResponseParts {
    (
        TypedHeader(headers::ContentType::from(mime::TEXT_CSS)),
        TypedHeader(headers::CacheControl::new().with_max_age(Duration::from_secs(3600))),
    )
}

fn favicon() -> impl IntoResponse {
    (
        TypedHeader(headers::ContentType::png()),
        TypedHeader(headers::CacheControl::new().with_max_age(Duration::from_secs(86400))),
        Bytes::from_static(include_bytes!("../assets/favicon.png")),
    )
}

pub fn routes() -> Router<Layer> {
    Router::new()
        .route("/", get(|| async { index() }).post(insert))
        .route("/:id", get(get_paste).delete(delete))
        .route("/burn/:id", get(|Path(id)| async { pages::Burn::new(id) }))
        .route("/delete/:id", get(delete))
        .route("/favicon.png", get(|| async { favicon() }))
        .route(
            "/style.css",
            get(|| async { (css_headers(), highlight::DATA.main.to_string()) }),
        )
        .route(
            "/dark.css",
            get(|| async { (css_headers(), highlight::DATA.dark.to_string()) }),
        )
        .route(
            "/light.css",
            get(|| async { (css_headers(), highlight::DATA.light.to_string()) }),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{make_app, Client};
    use http::StatusCode;
    use reqwest::header;

    #[tokio::test]
    async fn unknown_paste() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(make_app()?);

        let res = client.get("/000000").send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }

    #[tokio::test]
    async fn insert_via_form() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(make_app()?);

        let data = FormEntry {
            text: "FooBarBaz".to_string(),
            extension: Some("rs".to_string()),
            expires: "0".to_string(),
        };

        let res = client.post("/").form(&data).send().await?;
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
    async fn insert_via_json() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(make_app()?);

        let entry = Entry {
            text: "FooBarBaz".to_string(),
            ..Default::default()
        };

        let res = client.post("/").json(&entry).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let payload = res.json::<RedirectResponse>().await?;

        let res = client.get(&payload.path).send().await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await?, "FooBarBaz");

        let res = client.delete(&payload.path).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let res = client.get(&payload.path).send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }

    #[tokio::test]
    async fn delete_via_link() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(make_app()?);

        let data = FormEntry {
            text: "FooBarBaz".to_string(),
            extension: None,
            expires: "0".to_string(),
        };

        let res = client.post("/").form(&data).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let location = res.headers().get("location").unwrap().to_str()?;
        let res = client.get(&format!("/delete{location}")).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let res = client.get(location).send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }

    #[tokio::test]
    async fn delete_via_api() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(make_app()?);

        let entry = Entry {
            text: "FooBarBaz".to_string(),
            ..Default::default()
        };

        let res = client.post("/").json(&entry).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let payload = res.json::<RedirectResponse>().await?;

        let res = client.get(&payload.path).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let res = client.delete(&payload.path).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let res = client.get(&payload.path).send().await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        Ok(())
    }

    #[tokio::test]
    async fn download() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new(make_app()?);

        let data = FormEntry {
            text: "FooBarBaz".to_string(),
            extension: None,
            expires: "0".to_string(),
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
