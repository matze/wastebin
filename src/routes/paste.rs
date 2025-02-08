use crate::cache::{Cache, Key as CacheKey};
use crate::crypto::Password;
use crate::db::read::Entry;
use crate::pages::{self, make_error, Burn};
use crate::routes::{form, json};
use crate::{AppState, Database, Error, Highlighter, Page};
use axum::body::Body;
use axum::extract::{Form, Json, Path, Query, State};
use axum::http::header::{self, HeaderMap};
use axum::http::{Request, StatusCode};
use axum::response::{AppendHeaders, IntoResponse, Redirect, Response};
use axum::RequestExt;
use axum_extra::extract::cookie::SignedCookieJar;
use axum_extra::headers;
use axum_extra::headers::{HeaderMapExt, HeaderValue};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub enum Format {
    #[serde(rename(deserialize = "raw"))]
    Raw,
    #[serde(rename(deserialize = "qr"))]
    Qr,
    #[serde(rename(deserialize = "dl"))]
    Dl,
}

#[derive(Deserialize, Debug)]
pub struct QueryData {
    pub fmt: Option<Format>,
}

#[derive(Deserialize, Debug)]
pub struct PasswordForm {
    password: String,
}

fn qr_code_from(page: &Page, id: String, ext: Option<String>) -> Result<qrcodegen::QrCode, Error> {
    let name = if let Some(ext) = ext {
        format!("{id}.{ext}")
    } else {
        id
    };

    Ok(qrcodegen::QrCode::encode_text(
        page.base_url.join(&name)?.as_str(),
        qrcodegen::QrCodeEcc::High,
    )?)
}

async fn get_qr(
    page: Page,
    key: CacheKey,
    title: String,
) -> Result<pages::Qr, pages::ErrorResponse> {
    let err_page = page.clone();

    async {
        let id = key.id();
        let ext = key.ext.is_empty().then_some(key.ext.clone());

        let qr_code = {
            let page = page.clone();

            tokio::task::spawn_blocking(move || qr_code_from(&page, id, ext))
                .await
                .map_err(Error::from)??
        };

        Ok(pages::Qr::new(qr_code, key, title, page))
    }
    .await
    .map_err(|err| make_error(err, err_page))
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

async fn get_html(
    page: Page,
    cache: Cache,
    highlighter: Highlighter,
    key: CacheKey,
    entry: Entry,
    jar: SignedCookieJar,
    is_protected: bool,
) -> Result<impl IntoResponse, pages::ErrorResponse> {
    async {
        let can_delete = jar
            .get("uid")
            .map(|cookie| cookie.value().parse::<i64>())
            .transpose()
            .map_err(|err| Error::CookieParsing(err.to_string()))?
            .zip(entry.uid)
            .is_some_and(|(user_uid, owner_uid)| user_uid == owner_uid);

        if let Some(html) = cache.get(&key) {
            tracing::trace!(?key, "found cached item");
            return Ok(pages::Paste::new(
                key,
                html,
                can_delete,
                entry.title.unwrap_or_default(),
                page.clone(),
            )
            .into_response());
        }

        // TODO: turn this upside-down, i.e. cache it but only return a cached version if we were able
        // to decrypt the content. Highlighting is probably still much slower than decryption.
        let can_be_cached = !entry.must_be_deleted;
        let ext = key.ext.clone();
        let title = entry.title.clone().unwrap_or_default();
        let html = highlighter.highlight(entry, ext).await?;

        if can_be_cached && !is_protected {
            tracing::trace!(?key, "cache item");
            cache.put(key.clone(), html.clone());
        }

        Ok(pages::Paste::new(key, html, can_delete, title, page.clone()).into_response())
    }
    .await
    .map_err(|err| make_error(err, page))
}

#[expect(clippy::too_many_arguments)]
pub async fn get(
    State(cache): State<Cache>,
    State(page): State<Page>,
    State(db): State<Database>,
    State(highlighter): State<Highlighter>,
    Path(id): Path<String>,
    headers: HeaderMap,
    jar: SignedCookieJar,
    Query(query): Query<QueryData>,
    form: Option<Form<PasswordForm>>,
) -> Result<Response, pages::ErrorResponse> {
    async {
        let password = form
            .map(|form| form.password.clone())
            .or_else(|| {
                headers
                    .get("Wastebin-Password")
                    .and_then(|header| header.to_str().ok().map(std::string::ToString::to_string))
            })
            .map(|password| Password::from(password.as_bytes().to_vec()));
        let key: CacheKey = id.parse()?;

        match db.get(key.id, password.clone()).await {
            Err(Error::NoPassword) => {
                Ok(pages::Encrypted::new(key, &query, page.clone()).into_response())
            }
            Err(err) => Err(err),
            Ok(entry) => {
                if entry.must_be_deleted {
                    db.delete(key.id).await?;
                }

                match query.fmt {
                    Some(Format::Raw) => return Ok(entry.text.into_response()),
                    Some(Format::Qr) => {
                        return Ok(get_qr(
                            page.clone(),
                            key,
                            entry.title.clone().unwrap_or_default(),
                        )
                        .await
                        .into_response())
                    }
                    Some(Format::Dl) => {
                        return Ok(get_download(entry.text, &key.id(), &key.ext).into_response());
                    }
                    None => (),
                }

                if let Some(value) = headers.get(header::ACCEPT) {
                    if let Ok(value) = value.to_str() {
                        if value.contains("text/html") {
                            return Ok(get_html(
                                page.clone(),
                                cache,
                                highlighter,
                                key,
                                entry,
                                jar,
                                password.is_some(),
                            )
                            .await
                            .into_response());
                        }
                    }
                }

                Ok(entry.text.into_response())
            }
        }
    }
    .await
    .map_err(|err| make_error(err, page))
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
        let is_https = headers
            .get(http::header::HOST)
            .zip(headers.get(http::header::ORIGIN))
            .and_then(|(host, origin)| host.to_str().ok().zip(origin.to_str().ok()))
            .and_then(|(host, origin)| {
                origin
                    .strip_prefix("https://")
                    .map(|origin| origin.starts_with(host))
            })
            .unwrap_or(false);

        let entry: Form<form::Entry> = request
            .extract()
            .await
            .map_err(IntoResponse::into_response)?;

        Ok(form::insert(state, jar, entry, is_https)
            .await
            .into_response())
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
    State(db): State<Database>,
    State(page): State<Page>,
    jar: SignedCookieJar,
) -> Result<Redirect, pages::ErrorResponse> {
    async {
        let id = id.parse()?;
        let uid = db.get_uid(id).await?;
        let can_delete = jar
            .get("uid")
            .map(|cookie| cookie.value().parse::<i64>())
            .transpose()
            .map_err(|err| Error::CookieParsing(err.to_string()))?
            .zip(uid)
            .is_some_and(|(user_uid, db_uid)| user_uid == db_uid);

        if !can_delete {
            Err(Error::Delete)?;
        }

        db.delete(id).await?;

        Ok(Redirect::to("/"))
    }
    .await
    .map_err(|err| make_error(err, page.clone()))
}

pub async fn burn_created(
    Path(id): Path<String>,
    State(page): State<Page>,
) -> Result<Burn, pages::ErrorResponse> {
    async {
        let id_clone = id.clone();
        let qr_code = tokio::task::spawn_blocking({
            let page = page.clone();
            move || qr_code_from(&page, id, None)
        })
        .await
        .map_err(Error::from)??;

        Ok(Burn::new(qr_code, id_clone, page.clone()))
    }
    .await
    .map_err(|err| make_error(err, page))
}
