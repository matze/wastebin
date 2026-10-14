use url::Url;

use crate::assets::{Asset, Css, Kind};
use wastebin_core::expiration::{Expiration, ExpirationSet};
use wastebin_highlight::Theme;

/// Static page assets.
pub(crate) struct Assets {
    pub favicon: Asset,
    pub css: Css,
    pub index_js: Asset,
    pub paste_js: Asset,
    pub burn_js: Asset,
    pub password_toggle_js: Asset,
}

pub(crate) struct Page {
    pub version: &'static str,
    pub title: String,
    pub assets: Assets,
    pub base_url: Url,
    pub expirations: Vec<Expiration>,
    pub max_body_size: usize,
}

impl Page {
    /// Create new page meta data from generated  `assets`, `title` and optional `base_url`.
    #[must_use]
    pub fn new(
        title: String,
        base_url: Url,
        theme: Theme,
        expirations: ExpirationSet,
        max_body_size: usize,
    ) -> Self {
        let assets = Assets::new(theme);
        let expirations = expirations.into_inner();

        Self {
            version: env!("CARGO_PKG_VERSION"),
            title,
            assets,
            base_url,
            expirations,
            max_body_size,
        }
    }
}

impl Assets {
    /// Create page [`Assets`] for the given `theme`.
    fn new(theme: Theme) -> Self {
        Self {
            favicon: Asset::new(
                "favicon",
                Kind::Png,
                include_bytes!("../../../assets/favicon.png").to_vec(),
            ),
            css: Css::new(theme),
            index_js: Asset::new(
                "index",
                Kind::Js,
                include_bytes!("javascript/index.js").to_vec(),
            ),
            paste_js: Asset::new(
                "paste",
                Kind::Js,
                include_bytes!("javascript/paste.js").to_vec(),
            ),
            burn_js: Asset::new(
                "burn",
                Kind::Js,
                include_bytes!("javascript/burn.js").to_vec(),
            ),
            password_toggle_js: Asset::new(
                "password-toggle",
                Kind::Js,
                include_bytes!("javascript/password-toggle.js").to_vec(),
            ),
        }
    }
}
