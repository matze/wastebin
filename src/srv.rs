use crate::db::Database;
use crate::id::Id;
use crate::Error;
use askama::Template;
use axum::extract::{Form, Path};
use axum::http::{header, StatusCode};
use axum::response::Redirect;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use once_cell::sync::Lazy;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::convert::From;
use std::io::Cursor;
use syntect::highlighting::ThemeSet;
use syntect::html::{css_for_theme_with_class_style, ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

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

struct Styles<'a> {
    main: &'a str,
    dark: String,
    light: String,
}

static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(|| SyntaxSet::load_defaults_newlines());

static STYLES: Lazy<Styles> = Lazy::new(|| {
    let data = include_str!("themes/ayu-light.tmTheme");
    let light_theme = ThemeSet::load_from_reader(&mut Cursor::new(data)).unwrap();

    let data = include_str!("themes/ayu-dark.tmTheme");
    let dark_theme = ThemeSet::load_from_reader(&mut Cursor::new(data)).unwrap();

    Styles {
        main: include_str!("themes/style.css"),
        light: css_for_theme_with_class_style(&light_theme, ClassStyle::Spaced).unwrap(),
        dark: css_for_theme_with_class_style(&dark_theme, ClassStyle::Spaced).unwrap(),
    }
});

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
        syntaxes: SYNTAX_SET.syntaxes(),
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

    let url = match entry.extension {
        Some(ref ext) => format!("/{}.{}", id.as_str(), ext),
        None => format!("/{}", id.as_str()),
    };

    let burn_after_reading = entry.burn_after_reading.unwrap_or(false);

    // TODO: sanitize
    db.insert(&id, entry).await.unwrap();

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

    let data: Entry = db.get(&id).await?.into();

    let formatted = tokio::task::spawn_blocking(move || {
        let syntax_ref = match ext {
            Some(ext) => SYNTAX_SET
                .find_syntax_by_extension(&ext)
                .unwrap_or_else(|| SYNTAX_SET.find_syntax_by_extension("txt").unwrap()),
            None => SYNTAX_SET.find_syntax_by_extension("txt").unwrap(),
        };

        let mut generator =
            ClassedHTMLGenerator::new_with_class_style(syntax_ref, &SYNTAX_SET, ClassStyle::Spaced);

        for line in LinesWithEndings::from(&data.text) {
            generator
                .parse_html_for_line_which_includes_newline(line)
                .unwrap();
        }

        Ok::<String, Error>(generator.finalize())
    })
    .await
    .map_err(Error::from)??;

    let id = id.as_str().to_string();

    Ok(Paste { formatted, id })
}

async fn raw(Path(id): Path<String>, db: Extension<Database>) -> Result<String, ErrorHtml> {
    let data: Entry = db.get(&Id::try_from(id.as_str())?).await?.into();
    Ok(data.text)
}

pub fn new_router(db: Database) -> Router {
    Router::new()
        .route("/", get(index).post(insert_via_form))
        .route(
            "/style.css",
            get(|| async { ([(header::CONTENT_TYPE, "text/css")], STYLES.main) }),
        )
        .route(
            "/dark.css",
            get(|| async { ([(header::CONTENT_TYPE, "text/css")], STYLES.dark.clone()) }),
        )
        .route(
            "/light.css",
            get(|| async { ([(header::CONTENT_TYPE, "text/css")], STYLES.light.clone()) }),
        )
        .route("/:id", get(show))
        .route("/api/entries", post(insert_via_api))
        .route("/api/entries/:id", get(raw))
        .layer(Extension(db))
}
