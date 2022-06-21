use crate::cache::{Key, Layer};
use crate::highlight::{self, DATA};
use crate::id::Id;
use crate::{Entry, Error, Router};
use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Form, Path};
use axum::http::StatusCode;
use axum::response::Redirect;
use axum::routing::get;
use axum::{headers, Extension, TypedHeader};
use bytes::Bytes;
use once_cell::sync::Lazy;
use rand::Rng;
use serde::Deserialize;
use std::env;

pub static TITLE: Lazy<String> =
    Lazy::new(|| env::var("WASTEBIN_TITLE").unwrap_or_else(|_| "wastebin".to_string()));

#[derive(Debug, Deserialize)]
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
        }
    }
}

#[derive(Template)]
#[template(path = "index.html")]
struct Index<'a> {
    title: &'a str,
    syntaxes: &'a [syntect::parsing::SyntaxReference],
}

#[derive(Template)]
#[template(path = "paste.html")]
struct Paste<'a> {
    title: &'a str,
    id: String,
    formatted: String,
}

#[derive(Template)]
#[template(path = "burn.html")]
struct BurnPage<'a> {
    title: &'a str,
    id: String,
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorPage<'a> {
    title: &'a str,
    error: String,
}

type ErrorHtml<'a> = (StatusCode, ErrorPage<'a>);

impl From<Error> for ErrorHtml<'_> {
    fn from(err: Error) -> Self {
        let html = ErrorPage {
            title: &TITLE,
            error: err.to_string(),
        };

        (err.into(), html)
    }
}

#[allow(clippy::unused_async)]
async fn index<'a>() -> Index<'a> {
    Index {
        title: &TITLE,
        syntaxes: DATA.syntax_set.syntaxes(),
    }
}

async fn insert(
    Form(entry): Form<FormEntry>,
    layer: Extension<Layer>,
) -> Result<Redirect, ErrorHtml<'static>> {
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

async fn show(
    id_with_opt_ext: Path<String>,
    layer: Extension<Layer>,
) -> Result<Paste<'static>, ErrorHtml<'static>> {
    let title = &TITLE;
    let key = Key::try_from(id_with_opt_ext)?;
    let id = key.id();
    let formatted = layer.get_formatted(key).await?;

    Ok(Paste {
        title,
        id,
        formatted,
    })
}

#[allow(clippy::unused_async)]
async fn burn_link(Path(id): Path<String>) -> BurnPage<'static> {
    BurnPage { title: &TITLE, id }
}

#[allow(clippy::unused_async)]
async fn favicon() -> impl IntoResponse {
    (
        TypedHeader(headers::ContentType::png()),
        Bytes::from_static(include_bytes!("../assets/favicon.png")),
    )
}

pub fn routes() -> Router {
    Router::new()
        .route("/", get(index).post(insert))
        .route("/:id", get(show))
        .route("/burn/:id", get(burn_link))
        .route("/favicon.png", get(favicon))
        .route("/style.css", get(|| async { highlight::main() }))
        .route("/dark.css", get(|| async { highlight::dark() }))
        .route("/light.css", get(|| async { highlight::light() }))
}
