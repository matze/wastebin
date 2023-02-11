use crate::db::{CacheKey, InsertEntry};
use crate::id::Id;
use crate::AppState;
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
use axum_extra::extract::cookie::{Cookie, SignedCookieJar};
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

#[derive(Debug, Serialize, Deserialize)]
struct JsonEntry {
    text: String,
    extension: Option<String>,
    expires: Option<u32>,
    burn_after_reading: Option<bool>,
}

#[derive(Serialize)]
struct ErrorPayload {
    message: String,
}

#[derive(Deserialize, Serialize)]
struct RedirectResponse {
    path: String,
}

#[derive(Deserialize, Debug)]
struct GetQuery {
    fmt: Option<String>,
    dl: Option<String>,
}

type ErrorResponse = (StatusCode, Json<ErrorPayload>);

impl From<FormEntry> for InsertEntry {
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
            uid: None,
        }
    }
}

impl From<JsonEntry> for InsertEntry {
    fn from(entry: JsonEntry) -> Self {
        Self {
            text: entry.text,
            extension: entry.extension,
            expires: entry.expires,
            burn_after_reading: entry.burn_after_reading,
            uid: None,
        }
    }
}

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
    state: State<AppState>,
    jar: SignedCookieJar,
) -> Result<(SignedCookieJar, Redirect), pages::ErrorResponse<'static>> {
    let id: Id = tokio::task::spawn_blocking(|| {
        let mut rng = rand::thread_rng();
        rng.gen::<u32>()
    })
    .await
    .map_err(Error::from)?
    .into();

    // Retrieve uid from cookie or generate a new one.
    let uid = if let Some(cookie) = jar.get("uid") {
        cookie
            .value()
            .parse::<i64>()
            .map_err(|err| Error::CookieParsing(err.to_string()))?
    } else {
        state.db.next_uid().await?
    };

    let mut entry: InsertEntry = entry.into();
    entry.uid = Some(uid);

    let url = id.to_url_path(&entry);
    let burn_after_reading = entry.burn_after_reading.unwrap_or(false);

    state.db.insert(id, entry).await?;

    let jar = jar.add(Cookie::new("uid", uid.to_string()));

    if burn_after_reading {
        Ok((jar, Redirect::to(&format!("/burn{url}"))))
    } else {
        Ok((jar, Redirect::to(&url)))
    }
}

async fn insert_from_json(
    Json(entry): Json<JsonEntry>,
    state: State<AppState>,
) -> Result<Json<RedirectResponse>, ErrorResponse> {
    let id: Id = tokio::task::spawn_blocking(|| {
        let mut rng = rand::thread_rng();
        rng.gen::<u32>()
    })
    .await
    .map_err(Error::from)?
    .into();

    let entry: InsertEntry = entry.into();
    let path = id.to_url_path(&entry);

    state.db.insert(id, entry).await?;

    Ok(Json::from(RedirectResponse { path }))
}

async fn insert(
    state: State<AppState>,
    jar: SignedCookieJar,
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

        Ok(insert_from_form(entry, state, jar).await.into_response())
    } else if content_type == headers::ContentType::json() {
        let entry: Json<JsonEntry> = request
            .extract()
            .await
            .map_err(IntoResponse::into_response)?;

        Ok(insert_from_json(entry, state).await.into_response())
    } else {
        Err(StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response())
    }
}

async fn get_html(
    Path(id): Path<String>,
    state: AppState,
    jar: SignedCookieJar,
) -> Result<pages::Paste<'static>, pages::ErrorResponse<'static>> {
    let key: CacheKey = id.parse()?;
    let owner_uid = state.db.get_uid(key.id).await?;
    let html = state.db.get_html(&key).await?;
    let can_delete = jar
        .get("uid")
        .map(|cookie| cookie.value().parse::<i64>())
        .transpose()
        .map_err(|err| Error::CookieParsing(err.to_string()))?
        .zip(owner_uid)
        .map_or(false, |(user_uid, owner_uid)| user_uid == owner_uid);

    Ok(pages::Paste::new(key.id(), key.ext, html, can_delete))
}

async fn get_raw(Path(id): Path<String>, state: AppState) -> Result<String, ErrorResponse> {
    // Remove the extension and try to reconstruct the identifier.
    let id = id
        .find('.')
        .map_or(id.as_str(), |index| &id[..index])
        .parse()?;

    Ok(state.db.get(id).await?.text)
}

async fn get_download(
    Path(id): Path<String>,
    extension: String,
    state: AppState,
) -> Result<Response<String>, pages::ErrorResponse<'static>> {
    // Validate extension.
    if !extension.is_ascii() {
        Err(Error::IllegalCharacters)?;
    }

    let raw_string = state.db.get(id.parse()?).await?.text;
    let content_type = "text; charset=utf-8";
    let content_disposition = format!(r#"attachment; filename="{id}.{extension}"#);

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
        .header(header::CONTENT_DISPOSITION, content_disposition)
        .body(raw_string)
        .map_err(Error::from)?)
}

async fn get_paste(
    id: Path<String>,
    headers: HeaderMap,
    jar: SignedCookieJar,
    Query(query): Query<GetQuery>,
    State(state): State<AppState>,
) -> Response {
    if let Some(fmt) = query.fmt {
        if fmt == "raw" {
            return get_raw(id, state).await.into_response();
        }
    }

    if let Some(extension) = query.dl {
        return get_download(id, extension, state).await.into_response();
    }

    if let Some(value) = headers.get(header::ACCEPT) {
        if let Ok(value) = value.to_str() {
            if value.contains("text/html") {
                return get_html(id, state, jar).await.into_response();
            }
        }
    }

    get_raw(id, state).await.into_response()
}

async fn delete(
    Path(id): Path<String>,
    state: State<AppState>,
    jar: SignedCookieJar,
) -> Result<Redirect, pages::ErrorResponse<'static>> {
    let id = id.parse()?;
    let entry = state.db.get(id).await?;
    let can_delete = jar
        .get("uid")
        .map(|cookie| cookie.value().parse::<i64>())
        .transpose()
        .map_err(|err| Error::CookieParsing(err.to_string()))?
        .zip(entry.uid)
        .map_or(false, |(user_uid, db_uid)| user_uid == db_uid);

    if !can_delete {
        Err(Error::Delete)?;
    }

    state.db.delete(id).await?;

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

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(|| async { index() }).post(insert))
        .route("/:id", get(get_paste).delete(delete))
        .route("/burn/:id", get(|Path(id)| async { pages::Burn::new(id) }))
        .route("/delete/:id", get(delete))
        .route("/favicon.png", get(|| async { favicon() }))
        .route(
            "/style.css",
            get(|| async { (css_headers(), highlight::DATA.style_css()) }),
        )
        .route(
            "/dark.css",
            get(|| async { (css_headers(), highlight::DATA.dark_css()) }),
        )
        .route(
            "/light.css",
            get(|| async { (css_headers(), highlight::DATA.light_css()) }),
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

        let entry = InsertEntry {
            text: "FooBarBaz".to_string(),
            ..Default::default()
        };

        let res = client.post("/").json(&entry).send().await?;
        assert_eq!(res.status(), StatusCode::OK);

        let payload = res.json::<RedirectResponse>().await?;

        let res = client.get(&payload.path).send().await?;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await?, "FooBarBaz");

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
        let uid_cookie = res.cookies().find(|cookie| cookie.name() == "uid");
        assert!(uid_cookie.is_some());
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let location = res.headers().get("location").unwrap().to_str()?;
        let res = client.get(&format!("/delete{location}")).send().await?;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        let res = client.get(location).send().await?;
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
