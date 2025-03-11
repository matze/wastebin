pub mod burn;
pub mod index;
pub mod paste;
pub mod qr;

use crate::Page;
use crate::handlers::extract::Theme;
use askama::Template;
use axum::http::StatusCode;

/// Error page showing a message.
#[derive(Template)]
#[template(path = "error.html")]
pub(crate) struct Error {
    pub page: Page,
    pub theme: Option<Theme>,
    pub description: String,
}

/// Page showing password input.
#[derive(Template)]
#[template(path = "encrypted.html")]
pub(crate) struct PasswordInput {
    pub page: Page,
    pub theme: Option<Theme>,
    pub id: String,
}

/// Error response carrying a status code and the page itself.
pub(crate) type ErrorResponse = (StatusCode, Error);

/// Create an error response from `error` consisting of [`StatusCode`] derive from `error` as well
/// as a rendered page with a description.
pub fn make_error(error: crate::Error, page: Page, theme: Option<Theme>) -> ErrorResponse {
    let description = error.to_string();
    (
        error.into(),
        Error {
            page,
            theme,
            description,
        },
    )
}
