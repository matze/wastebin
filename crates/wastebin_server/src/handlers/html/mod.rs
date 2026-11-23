pub mod burn;
pub mod document;
pub mod index;
pub mod paste;
pub mod qr;
pub mod rendered;

use std::fmt;

use askama::Template;
use askama_web::WebTemplate;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::cache::Key;
use crate::handlers::NoStore;
use crate::handlers::extract::Theme;
use crate::handlers::html::paste::PasteForm;
use crate::i18n::Lang;
use crate::{Database, Page};
use wastebin_core::crypto::Password;
use wastebin_core::db;
use wastebin_core::db::read::{Data, Entry};

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

    /// Post back to the HTML document view at `/html/{key}`.
    pub(crate) fn html(key: &Key) -> Self {
        Self(format!("/html/{key}"))
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

/// A paste the viewer has confirmed burning and supplied the password for, where needed.
pub(crate) struct Unlocked {
    pub data: Data,
    /// If the paste is still in the database and can be fetched with another request.
    pub is_available: bool,
    pub encrypted: bool,
    pub no_password: bool,
}

/// Outcome of [`unlock`]: the paste, or the interstitial page to show before it.
pub(crate) enum Unlock {
    Paste(Unlocked),
    Prompt(Response),
}

/// Fetch the paste behind `key`, unless it first needs a burn confirmation or a password. Both
/// prompts post back to `action` with the answer in `form`.
pub(crate) async fn unlock(
    db: &Database,
    page: &Page,
    theme: Option<&Theme>,
    lang: Lang,
    key: &Key,
    form: Option<PasteForm>,
    action: FormAction,
) -> Result<Unlock, crate::Error> {
    let password = form
        .as_ref()
        .and_then(|form| form.password.as_ref())
        .filter(|password| !password.is_empty())
        .map(|password| Password::from(password.as_bytes().to_vec()));
    let confirmed = form.as_ref().and_then(|form| form.confirm_burn.as_deref()) == Some("1");
    let no_password = password.is_none();
    let metadata = db.get_metadata(key.id).await?;

    if metadata.must_be_deleted && !confirmed {
        return Ok(Unlock::Prompt(
            BurnConfirmation {
                page: page.clone(),
                theme: theme.cloned(),
                lang,
                title: metadata.title,
                action,
            }
            .into_response(),
        ));
    }

    let (data, is_available, encrypted) = match db.get(key.id, password).await {
        Ok(Entry::Regular(data)) => (data, true, false),
        Ok(Entry::Burned(data)) => (data, false, false),
        Ok(Entry::Encrypted(data)) => {
            let is_available = !data.metadata.must_be_deleted;
            (data, is_available, true)
        }
        Err(db::Error::NoPassword) => {
            return Ok(Unlock::Prompt(
                PasswordInput {
                    page: page.clone(),
                    theme: theme.cloned(),
                    lang,
                    action,
                }
                .into_response()
                .no_store(),
            ));
        }
        Err(err) => return Err(err.into()),
    };

    Ok(Unlock::Paste(Unlocked {
        data,
        is_available,
        encrypted,
        no_password,
    }))
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
