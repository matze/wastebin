use crate::db::Database;
use crate::highlight::DATA;
use crate::id::Id;
use crate::Error;
use askama::Template;
use axum::extract::{Form, Path};
use axum::http::StatusCode;
use axum::response::Redirect;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::convert::From;

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    /// Content
    pub text: String,
    /// File extension
    pub extension: Option<String>,
    /// Expiration in seconds from now
    pub expires: Option<u32>,
    /// Delete if read
    pub burn_after_reading: Option<bool>,
}

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

#[derive(Serialize)]
struct ErrorPayload {
    message: String,
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorPage {
    error: String,
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

type ErrorResponse = (StatusCode, Json<ErrorPayload>);

impl From<Error> for ErrorResponse {
    fn from(err: Error) -> Self {
        let payload = Json::from(ErrorPayload {
            message: err.to_string(),
        });

        (err.into(), payload)
    }
}

type ErrorHtml = (StatusCode, ErrorPage);

impl From<Error> for StatusCode {
    fn from(err: Error) -> Self {
        match err {
            Error::Sqlite(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Migration(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::TimeFormatting(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::IllegalCharacters => StatusCode::BAD_REQUEST,
            Error::WrongSize => StatusCode::BAD_REQUEST,
            Error::Join(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Syntax(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

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

async fn insert(entry: Entry, db: Extension<Database>) -> Redirect {
    let id: Id = tokio::task::spawn_blocking(|| {
        let mut rng = rand::thread_rng();
        rng.gen::<u32>()
    })
    .await
    .unwrap()
    .into();

    let id_string = id.to_string();

    let url = match entry.extension {
        Some(ref ext) => format!("/{}.{}", id_string, ext),
        None => format!("/{}", id_string),
    };

    let burn_after_reading = entry.burn_after_reading.unwrap_or(false);

    // TODO: sanitize
    db.insert(id, entry).await.unwrap();

    if burn_after_reading {
        Redirect::to("/")
    } else {
        Redirect::to(&url)
    }
}

async fn insert_via_form(Form(entry): Form<FormEntry>, db: Extension<Database>) -> Redirect {
    insert(entry.into(), db).await
}

async fn insert_via_api(Json(entry): Json<Entry>, db: Extension<Database>) -> Redirect {
    insert(entry, db).await
}

async fn show(
    Path(id_with_opt_ext): Path<String>,
    db: Extension<Database>,
) -> Result<Paste, ErrorHtml> {
    let (id, ext) = match id_with_opt_ext.split_once('.') {
        None => (Id::try_from(id_with_opt_ext.as_str())?, None),
        Some((id, ext)) => (Id::try_from(id)?, Some(ext.to_string())),
    };

    let entry = db.get(id).await?;
    let id = id.to_string();

    let formatted = tokio::task::spawn_blocking(move || DATA.highlight(entry, ext))
        .await
        .map_err(Error::from)??;

    Ok(Paste { formatted, id })
}

async fn raw(Path(id): Path<String>, db: Extension<Database>) -> Result<String, ErrorHtml> {
    Ok(db.get(Id::try_from(id.as_str())?).await?.text)
}

pub fn new_router(db: Database) -> Router {
    Router::new()
        .route("/", get(index).post(insert_via_form))
        .route("/style.css", get(|| async { DATA.main().await }))
        .route("/dark.css", get(|| async { DATA.dark().await }))
        .route("/light.css", get(|| async { DATA.light().await }))
        .route("/:id", get(show))
        .route("/api/entries", post(insert_via_api))
        .route("/api/entries/:id", get(raw))
        .layer(Extension(db))
}
