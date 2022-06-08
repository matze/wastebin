use crate::db::Database;
use crate::highlight::DATA;
use crate::id::Id;
use crate::{Cache, Entry, Error};
use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Form, Path};
use axum::http::StatusCode;
use axum::response::Redirect;
use axum::routing::get;
use axum::{headers, Extension, Router, TypedHeader};
use bytes::Bytes;
use rand::Rng;
use serde::Deserialize;

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
            Err(_) => None,
            Ok(0) => None,
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
    syntaxes: &'a [syntect::parsing::SyntaxReference],
}

#[derive(Template)]
#[template(path = "paste.html")]
struct Paste {
    id: String,
    formatted: String,
}

#[derive(Template)]
#[template(path = "burn.html")]
struct BurnPage {
    id: String,
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorPage {
    error: String,
}

type ErrorHtml = (StatusCode, ErrorPage);

impl From<Error> for ErrorHtml {
    fn from(err: Error) -> Self {
        let html = ErrorPage {
            error: err.to_string(),
        };

        (err.into(), html)
    }
}

async fn index<'a>() -> Index<'a> {
    Index {
        syntaxes: DATA.syntax_set.syntaxes(),
    }
}

async fn insert(
    Form(entry): Form<FormEntry>,
    db: Extension<Database>,
) -> Result<Redirect, ErrorHtml> {
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

    db.insert(id, entry).await?;

    if burn_after_reading {
        Ok(Redirect::to(&format!("/burn{url}")))
    } else {
        Ok(Redirect::to(&url))
    }
}

async fn show(
    Path(id_with_opt_ext): Path<String>,
    db: Extension<Database>,
    cache: Extension<Cache>,
) -> Result<Paste, ErrorHtml> {
    let (id, ext) = match id_with_opt_ext.split_once('.') {
        None => (Id::try_from(id_with_opt_ext.as_str())?, None),
        Some((id, ext)) => (Id::try_from(id)?, Some(ext.to_string())),
    };

    if let Some(cached) = cache.lock().unwrap().get(&id_with_opt_ext) {
        tracing::debug!(id = %id_with_opt_ext, "Found cached item");

        return Ok(Paste {
            formatted: cached.to_string(),
            id: id.to_string(),
        });
    }

    let entry = db.get(id).await?;
    let id = id.to_string();

    let formatted = tokio::task::spawn_blocking(move || DATA.highlight(entry, ext))
        .await
        .map_err(Error::from)??;

    tracing::debug!(id = %id_with_opt_ext, "No cached item");

    cache
        .lock()
        .unwrap()
        .put(id_with_opt_ext, formatted.clone());

    Ok(Paste { formatted, id })
}

async fn burn_link(Path(id): Path<String>) -> BurnPage {
    BurnPage { id }
}

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
        .route("/style.css", get(|| async { DATA.main().await }))
        .route("/dark.css", get(|| async { DATA.dark().await }))
        .route("/light.css", get(|| async { DATA.light().await }))
}
