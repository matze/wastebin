pub mod burn;
pub mod index;
pub mod paste;
pub mod qr;

pub use burn::burn;
pub use index::index;
pub use qr::qr;

use crate::{errors, Page};
use askama::Template;
use axum::http::StatusCode;

/// Error page showing a message.
#[derive(Template)]
#[template(path = "error.html")]
pub struct Error {
    pub page: Page,
    pub description: String,
}

/// Page showing password input.
#[derive(Template)]
#[template(path = "encrypted.html")]
pub struct PasswordInput {
    pub page: Page,
    pub id: String,
}

/// Error response carrying a status code and the page itself.
pub type ErrorResponse = (StatusCode, Error);

/// Create an error response from `error` consisting of [`StatusCode`] derive from `error` as well
/// as a rendered page with a description.
pub fn make_error(error: errors::Error, page: Page) -> ErrorResponse {
    let description = error.to_string();
    (error.into(), Error { page, description })
}
