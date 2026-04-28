pub mod burn;
pub mod index;
pub mod paste;
pub mod qr;
pub mod rendered;

use askama::Template;
use askama_web::WebTemplate;
use axum::http::StatusCode;

use crate::Page;
use crate::handlers::extract::Theme;
use crate::i18n::Lang;

/// Error page showing a message.
#[derive(Template, WebTemplate)]
#[template(path = "error.html")]
pub(crate) struct Error {
    pub page: Page,
    pub theme: Option<Theme>,
    pub lang: Lang,
    pub description: String,
}

/// Page showing password input.
#[derive(Template, WebTemplate)]
#[template(path = "encrypted.html")]
pub(crate) struct PasswordInput {
    pub page: Page,
    pub theme: Option<Theme>,
    pub lang: Lang,
    pub id: String,
}

/// Interstitial page shown before a burn-after-reading paste is revealed.
#[derive(Template, WebTemplate)]
#[template(path = "burn-confirmation.html")]
pub(crate) struct BurnConfirmation {
    pub page: Page,
    pub theme: Option<Theme>,
    pub lang: Lang,
    pub id: String,
    pub title: Option<String>,
}

/// Error response carrying a status code and the page itself.
pub(crate) type ErrorResponse = (StatusCode, Error);

/// Create an error response from `error` consisting of [`StatusCode`] derive from `error` as well
/// as a rendered page with a description.
pub fn make_error(
    error: crate::Error,
    page: Page,
    theme: Option<Theme>,
    lang: Lang,
) -> ErrorResponse {
    let description = error.to_string();
    (
        error.into(),
        Error {
            page,
            theme,
            lang,
            description,
        },
    )
}
