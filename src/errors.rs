use axum::Json;
use axum::http::StatusCode;
use serde::Serialize;
use std::num::TryFromIntError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("axum http error: {0}")]
    Axum(#[from] axum::http::Error),
    #[error("not allowed to delete")]
    Delete,
    #[error("compression error: {0}")]
    Compression(String),
    #[error("entry not found")]
    NotFound,
    #[error("sqlite error: {0}")]
    Sqlite(rusqlite::Error),
    #[error("migrations error: {0}")]
    Migration(#[from] rusqlite_migration::Error),
    #[error("wrong size")]
    WrongSize,
    #[error("illegal characters")]
    IllegalCharacters,
    #[error("integer conversion error: {0}")]
    IntConversion(#[from] TryFromIntError),
    #[error("join error: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("syntax highlighting error: {0}")]
    SyntaxHighlighting(#[from] syntect::Error),
    #[error("syntax parsing error: {0}")]
    SyntaxParsing(#[from] syntect::parsing::ParsingError),
    #[error("could not parse cookie: {0}")]
    CookieParsing(String),
    #[error("could not generate QR code: {0}")]
    QrCode(#[from] qrcodegen::DataTooLong),
    #[error("could not find Host header to generate QR code URL")]
    NoHost,
    #[error("could not parse URL: {0}")]
    UrlParsing(#[from] url::ParseError),
    #[error("argon2 error: {0}")]
    Argon2(#[from] argon2::Error),
    #[error("encryption failed")]
    ChaCha20Poly1305Encrypt,
    #[error("decryption failed")]
    ChaCha20Poly1305Decrypt,
    #[error("password not given")]
    NoPassword,
}

#[derive(Serialize)]
pub struct JsonError {
    pub message: String,
}

/// Response carrying a status code and the error message as JSON.
pub type JsonErrorResponse = (StatusCode, Json<JsonError>);

impl From<Error> for StatusCode {
    fn from(err: Error) -> Self {
        match err {
            Error::NotFound => StatusCode::NOT_FOUND,
            Error::NoHost
            | Error::IllegalCharacters
            | Error::WrongSize
            | Error::UrlParsing(_)
            | Error::NoPassword
            | Error::CookieParsing(_) => StatusCode::BAD_REQUEST,
            Error::Join(_)
            | Error::QrCode(_)
            | Error::Compression(_)
            | Error::IntConversion(_)
            | Error::Migration(_)
            | Error::Sqlite(_)
            | Error::SyntaxHighlighting(_)
            | Error::SyntaxParsing(_)
            | Error::Argon2(_)
            | Error::ChaCha20Poly1305Encrypt
            | Error::Axum(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Delete | Error::ChaCha20Poly1305Decrypt => StatusCode::FORBIDDEN,
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

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        match err {
            rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
            _ => Error::Sqlite(err),
        }
    }
}
