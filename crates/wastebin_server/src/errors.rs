use axum::Json;
use axum::http::StatusCode;
use serde::Serialize;
use std::num::TryFromIntError;
use wastebin_core::{crypto, db, id};

#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {
    #[error("axum http error: {0}")]
    Axum(#[from] axum::http::Error),
    #[error("integer conversion error: {0}")]
    IntConversion(#[from] TryFromIntError),
    #[error("join error: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("syntax highlighting error: {0}")]
    SyntaxHighlighting(#[from] syntect::Error),
    #[error("syntax parsing error: {0}")]
    SyntaxParsing(#[from] syntect::parsing::ParsingError),
    #[error("could not generate QR code: {0}")]
    QrCode(#[from] qrcodegen::DataTooLong),
    #[error("could not parse URL: {0}")]
    UrlParsing(#[from] url::ParseError),
    #[error("database error: {0}")]
    Database(#[from] db::Error),
    #[error("id error: {0}")]
    Id(#[from] id::Error),
    #[error("malformed form data")]
    MalformedForm,
    #[error("rate-limit hit")]
    RateLimit,
}

#[derive(Serialize)]
pub(crate) struct JsonError {
    pub message: String,
}

/// Response carrying a status code and the error message as JSON.
pub(crate) type JsonErrorResponse = (StatusCode, Json<JsonError>);

impl From<Error> for StatusCode {
    fn from(err: Error) -> Self {
        match err {
            Error::Database(db::Error::NoPassword) => StatusCode::BAD_REQUEST,
            Error::Database(db::Error::NotFound) => StatusCode::NOT_FOUND,
            Error::Database(
                db::Error::Delete | db::Error::Crypto(crypto::Error::ChaCha20Poly1305Decrypt),
            )
            | Error::RateLimit => StatusCode::FORBIDDEN,
            Error::Id(_) | Error::UrlParsing(_) => StatusCode::BAD_REQUEST,
            Error::MalformedForm => StatusCode::UNPROCESSABLE_ENTITY,
            Error::Join(_)
            | Error::QrCode(_)
            | Error::Database(_)
            | Error::IntConversion(_)
            | Error::SyntaxHighlighting(_)
            | Error::SyntaxParsing(_)
            | Error::Axum(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<Error> for JsonErrorResponse {
    fn from(err: Error) -> Self {
        let payload = Json::from(JsonError {
            message: err.to_string(),
        });

        (err.into(), payload)
    }
}
