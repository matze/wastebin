use crate::db::CacheKey;
use crate::routes::{form, json};
use crate::{pages, AppState, Error};
use axum::body::Body;
use axum::extract::{Form, Json, Path, Query, State};
use axum::headers::{self, HeaderMapExt, HeaderValue};
use axum::http::header::{self, HeaderMap};
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::RequestExt;
use axum_extra::extract::cookie::SignedCookieJar;
use serde::Deserialize;
use url::Url;

#[derive(Deserialize, Debug)]
enum Format {
    #[serde(rename(deserialize = "raw"))]
    Raw,
    #[serde(rename(deserialize = "qr"))]
    Qr,
}

#[derive(Deserialize, Debug)]
pub struct QueryData {
    fmt: Option<Format>,
    dl: Option<String>,
}

async fn get_raw(state: AppState, Path(id): Path<String>) -> Result<String, StatusCode> {
    // Remove the extension and try to reconstruct the identifier.
    let id = id
        .find('.')
        .map_or(id.as_str(), |index| &id[..index])
        .parse()?;

    Ok(state.db.get(id).await?.text)
}

fn qr_code_from(
    state: AppState,
    headers: &HeaderMap,
    id: &str,
) -> Result<qrcodegen::QrCode, Error> {
    let base_url = &state.base_url.map_or_else(
        || {
            // Fall back to the user agent's `Host` header field.
            let host = headers
                .get(header::HOST)
                .ok_or_else(|| Error::NoHost)?
                .to_str()
                .map_err(|_| Error::IllegalCharacters)?;

            Ok::<_, Error>(Url::parse(&format!("https://{host}"))?)
        },
        Ok,
    )?;

    Ok(qrcodegen::QrCode::encode_text(
        base_url.join(id)?.as_str(),
        qrcodegen::QrCodeEcc::High,
    )?)
}

async fn get_qr(
    state: AppState,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<pages::Qr<'static>, pages::ErrorResponse<'static>> {
    let qr_code = tokio::task::spawn_blocking(move || qr_code_from(state, &headers, &id))
        .await
        .map_err(Error::from)??;

    Ok(pages::Qr::new(qr_code))
}

async fn get_download(
    state: AppState,
    Path(id): Path<String>,
    extension: String,
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

    Ok(pages::Paste::new(key, html, can_delete))
}

pub async fn get(
    id: Path<String>,
    headers: HeaderMap,
    jar: SignedCookieJar,
    Query(query): Query<QueryData>,
    State(state): State<AppState>,
) -> Response {
    match query.fmt {
        Some(Format::Raw) => return get_raw(state, id).await.into_response(),
        Some(Format::Qr) => return get_qr(state, id, headers).await.into_response(),
        None => (),
    }

    if let Some(extension) = query.dl {
        return get_download(state, id, extension).await.into_response();
    }

    if let Some(value) = headers.get(header::ACCEPT) {
        if let Ok(value) = value.to_str() {
            if value.contains("text/html") {
                return get_html(id, state, jar).await.into_response();
            }
        }
    }

    get_raw(state, id).await.into_response()
}

pub async fn insert(
    state: State<AppState>,
    jar: SignedCookieJar,
    headers: HeaderMap,
    request: Request<Body>,
) -> Result<Response, Response> {
    let content_type = headers
        .typed_get::<headers::ContentType>()
        .ok_or_else(|| StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response())?;

    if content_type == headers::ContentType::form_url_encoded() {
        let entry: Form<form::Entry> = request
            .extract()
            .await
            .map_err(IntoResponse::into_response)?;

        Ok(form::insert(state, jar, entry).await.into_response())
    } else if content_type == headers::ContentType::json() {
        let entry: Json<json::Entry> = request
            .extract()
            .await
            .map_err(IntoResponse::into_response)?;

        Ok(json::insert(state, entry).await.into_response())
    } else {
        Err(StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response())
    }
}

pub async fn delete(
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
