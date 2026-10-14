pub mod burn;
pub mod index;
pub mod paste;
pub mod qr;
pub mod rendered;

use std::fmt;

use askama::Template;
use askama_web::WebTemplate;
use axum::http::StatusCode;

use crate::Page;
use crate::cache::Key;
use crate::handlers::extract::Theme;
use crate::i18n::Lang;

/// Target of an interstitial's submit form, i.e. the view the paste was requested from.
#[derive(Clone, Debug)]
pub(crate) struct FormAction(String);

impl FormAction {
    /// Post back to the source view at `/{key}`.
    pub(crate) fn source(key: &Key) -> Self {
        Self(format!("/{key}"))
    }

    /// Post back to the rendered Markdown view at `/md/{key}`.
    pub(crate) fn rendered(key: &Key) -> Self {
        Self(format!("/md/{key}"))
    }
}

impl fmt::Display for FormAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Error page showing a message.
#[derive(Template, WebTemplate)]
#[template(path = "error.html")]
pub(crate) struct Error {
    pub page: Page,
    pub theme: Option<Theme>,
    pub lang: Lang,
    pub description: String,
}

/// Page showing password input. Its form posts back to `action`, the view the user came from.
#[derive(Template, WebTemplate)]
#[template(path = "encrypted.html")]
pub(crate) struct PasswordInput {
    pub page: Page,
    pub theme: Option<Theme>,
    pub lang: Lang,
    pub action: FormAction,
}

/// Interstitial page shown before a burn-after-reading paste is revealed. Its form posts back to
/// `action`, the view the user came from.
#[derive(Template, WebTemplate)]
#[template(path = "burn-confirmation.html")]
pub(crate) struct BurnConfirmation {
    pub page: Page,
    pub theme: Option<Theme>,
    pub lang: Lang,
    pub title: Option<String>,
    pub action: FormAction,
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
