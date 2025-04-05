use crate::cache::Key;
use crate::handlers::extract::Theme;
use crate::handlers::html::{ErrorResponse, make_error};
use crate::{Error, Page};
use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};
use qrcodegen::QrCode;
use url::Url;
use wastebin_core::db::Database;

/// GET handler for a QR page.
pub async fn get(
    Path(id): Path<String>,
    State(page): State<Page>,
    State(db): State<Database>,
    theme: Option<Theme>,
) -> Result<Qr, ErrorResponse> {
    async {
        let key: Key = id.parse()?;

        let code = {
            let page = page.clone();

            tokio::task::spawn_blocking(move || code_from(&page.base_url, &id))
                .await
                .map_err(Error::from)??
        };

        let title = db.get_title(key.id.clone()).await?;

        // TODO: fix the bogus hardcoded can_delete and is_deleted fields.
        Ok(Qr {
            page: page.clone(),
            theme: theme.clone(),
            key,
            can_delete: false,
            is_available: false,
            code,
            title,
        })
    }
    .await
    .map_err(|err| make_error(err, page, theme))
}

/// Paste view showing the formatted paste as well as a bunch of links.
#[derive(Template, WebTemplate)]
#[template(path = "qr.html", escape = "none")]
pub(crate) struct Qr {
    page: Page,
    theme: Option<Theme>,
    key: Key,
    can_delete: bool,
    is_available: bool,
    code: qrcodegen::QrCode,
    title: Option<String>,
}

impl Qr {
    fn dark_modules(&self) -> Vec<(i32, i32)> {
        dark_modules(&self.code)
    }
}

pub fn code_from(url: &Url, id: &str) -> Result<QrCode, Error> {
    Ok(QrCode::encode_text(
        url.join(id)?.as_str(),
        qrcodegen::QrCodeEcc::High,
    )?)
}

/// Return module coordinates that are dark.
pub fn dark_modules(code: &QrCode) -> Vec<(i32, i32)> {
    let size = code.size();
    (0..size)
        .flat_map(|x| (0..size).map(move |y| (x, y)))
        .filter(|(x, y)| code.get_module(*x, *y))
        .collect()
}
