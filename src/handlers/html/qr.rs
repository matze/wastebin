use crate::cache::Key;
use crate::db::Database;
use crate::handlers::html::{make_error, ErrorResponse};
use crate::{Error, Page};
use askama::Template;
use axum::extract::{Path, State};
use qrcodegen::QrCode;
use url::Url;

/// GET handler for a QR page.
pub async fn qr(
    Path(id): Path<String>,
    State(page): State<Page>,
    State(db): State<Database>,
) -> Result<Qr, ErrorResponse> {
    async {
        let code = {
            let page = page.clone();
            let id = id.clone();

            tokio::task::spawn_blocking(move || code_from(&page.base_url, &id))
                .await
                .map_err(Error::from)??
        };

        let key: Key = id.parse()?;
        let title = db.get_title(key.id).await?.unwrap_or_default();

        Ok(Qr {
            page: page.clone(),
            key,
            can_delete: false,
            code,
            title,
        })
    }
    .await
    .map_err(|err| make_error(err, page.clone()))
}

/// Paste view showing the formatted paste as well as a bunch of links.
#[derive(Template)]
#[template(path = "qr.html", escape = "none")]
pub struct Qr {
    page: Page,
    key: Key,
    can_delete: bool,
    code: qrcodegen::QrCode,
    title: String,
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
