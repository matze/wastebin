use crate::cache::Key as CacheKey;
use crate::crypto::Password;
use crate::db::read::Entry;
use crate::highlight::Html;
use crate::routes::{form, json};
use crate::{pages, AppState, Error};
use axum::body::Body;
use axum::extract::{Form, Json, Path, Query, State};
use axum::headers::{self, HeaderMapExt, HeaderValue};
use axum::http::header::{self, HeaderMap};
use axum::http::{Request, StatusCode};
use axum::response::{AppendHeaders, IntoResponse, Redirect, Response};
use axum::RequestExt;
use axum_extra::extract::cookie::SignedCookieJar;
use serde::Deserialize;
use url::Url;

#[derive(Deserialize, Debug)]
pub enum Format {
    #[serde(rename(deserialize = "raw"))]
    Raw,
    #[serde(rename(deserialize = "qr"))]
    Qr,
}

#[derive(Deserialize, Debug)]
pub struct QueryData {
    pub fmt: Option<Format>,
    pub dl: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct PasswordForm {
    password: String,
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
    key: CacheKey,
    headers: HeaderMap,
) -> Result<pages::Qr<'static>, pages::ErrorResponse<'static>> {
    let id = key.id();
    let qr_code = tokio::task::spawn_blocking(move || qr_code_from(state, &headers, &id))
        .await
        .map_err(Error::from)??;

    Ok(pages::Qr::new(qr_code, key))
}

fn get_download(
    text: String,
    id: &str,
    extension: &str,
) -> Result<impl IntoResponse, pages::ErrorResponse<'static>> {
    // Validate extension.
    if !extension.is_ascii() {
        Err(Error::IllegalCharacters)?;
    }

    let content_type = "text; charset=utf-8";
    let content_disposition =
        HeaderValue::from_str(&format!(r#"attachment; filename="{id}.{extension}"#))
            .expect("constructing valid header value");

    Ok((
        AppendHeaders([
            (header::CONTENT_TYPE, HeaderValue::from_static(content_type)),
            (header::CONTENT_DISPOSITION, content_disposition),
        ]),
        text,
    ))
}

async fn get_html(
    state: AppState,
    key: CacheKey,
    entry: Entry,
    jar: SignedCookieJar,
    is_protected: bool,
) -> Result<impl IntoResponse, pages::ErrorResponse<'static>> {
    let can_delete = jar
        .get("uid")
        .map(|cookie| cookie.value().parse::<i64>())
        .transpose()
        .map_err(|err| Error::CookieParsing(err.to_string()))?
        .zip(entry.uid)
        .map_or(false, |(user_uid, owner_uid)| user_uid == owner_uid);

    if let Some(html) = state.cache.get(&key) {
        tracing::trace!(?key, "found cached item");
        return Ok(pages::Paste::new(key, html, can_delete).into_response());
    }

    // TODO: turn this upside-down, i.e. cache it but only return a cached version if we were able
    // to decrypt the content. Highlighting is probably still much slower than decryption.
    let can_be_cached = !entry.must_be_deleted;
    let ext = key.ext.clone();
    let html = Html::from(entry, ext).await?;

    if can_be_cached && !is_protected {
        tracing::trace!(?key, "cache item");
        state.cache.put(key.clone(), html.clone());
    }

    Ok(pages::Paste::new(key, html, can_delete).into_response())
}

pub async fn get(
    Path(id): Path<String>,
    headers: HeaderMap,
    jar: SignedCookieJar,
    Query(query): Query<QueryData>,
    State(state): State<AppState>,
    form: Option<Form<PasswordForm>>,
) -> Result<Response, pages::ErrorResponse<'static>> {
    let password = form
        .map(|form| form.password.clone())
        .or_else(|| {
            headers
                .get("Wastebin-Password")
                .and_then(|header| header.to_str().ok().map(std::string::ToString::to_string))
        })
        .map(|password| Password::from(password.as_bytes().to_vec()));
    let key: CacheKey = id.parse()?;

    match state.db.get(key.id, password.clone()).await {
        Err(Error::NoPassword) => Ok(pages::Encrypted::new(key, query).into_response()),
        Err(err) => Err(err.into()),
        Ok(entry) => {
            match query.fmt {
                Some(Format::Raw) => return Ok(entry.text.into_response()),
                Some(Format::Qr) => return Ok(get_qr(state, key, headers).await.into_response()),
                None => (),
            }

            if let Some(extension) = query.dl {
                return Ok(get_download(entry.text, &key.id(), &extension).into_response());
            }

            if let Some(value) = headers.get(header::ACCEPT) {
                if let Ok(value) = value.to_str() {
                    if value.contains("text/html") {
                        return Ok(get_html(state, key, entry, jar, password.is_some())
                            .await
                            .into_response());
                    }
                }
            }

            Ok(entry.text.into_response())
        }
    }
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
    let uid = state.db.get_uid(id).await?;
    let can_delete = jar
        .get("uid")
        .map(|cookie| cookie.value().parse::<i64>())
        .transpose()
        .map_err(|err| Error::CookieParsing(err.to_string()))?
        .zip(uid)
        .map_or(false, |(user_uid, db_uid)| user_uid == db_uid);

    if !can_delete {
        Err(Error::Delete)?;
    }

    state.db.delete(id).await?;

    Ok(Redirect::to("/"))
}
