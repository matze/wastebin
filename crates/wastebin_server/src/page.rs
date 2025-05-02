use std::num::NonZeroU32;

use crate::assets::{Asset, Css, Kind};
use crate::expiration::{Expiration, ExpirationSet};
use crate::highlight::Theme;
use url::Url;

/// Static page assets.
pub(crate) struct Assets {
    pub favicon: Asset,
    pub css: Css,
    pub base_js: Asset,
    pub index_js: Asset,
    pub paste_js: Asset,
}

pub(crate) struct Page {
    pub version: &'static str,
    pub title: String,
    pub assets: Assets,
    pub base_url: Url,
    pub expirations: Vec<Expiration>,
    pub max_expiration: Option<NonZeroU32>,
}

impl Page {
    /// Create new page meta data from generated  `assets`, `title` and optional `base_url`.
    #[must_use]
    pub fn new(
        title: String,
        base_url: Url,
        theme: Theme,
        expirations: ExpirationSet,
        max_expiration: Option<NonZeroU32>,
    ) -> Self {
        let assets = Assets::new(theme);
        let expirations = expirations.into_inner();

        Self {
            version: env!("CARGO_PKG_VERSION"),
            title,
            assets,
            base_url,
            expirations,
            max_expiration,
        }
    }
}

impl Assets {
    /// Create page [`Assets`] for the given `theme`.
    fn new(theme: Theme) -> Self {
        Self {
            favicon: Asset::new(
                "favicon.ico",
                mime::IMAGE_PNG,
                include_bytes!("../../../assets/favicon.png").to_vec(),
            ),
            css: Css::new(theme),
            base_js: Asset::new_hashed(
                "base",
                Kind::Js,
                include_bytes!("javascript/base.js").to_vec(),
            ),
            index_js: Asset::new_hashed(
                "index",
                Kind::Js,
                include_bytes!("javascript/index.js").to_vec(),
            ),
            paste_js: Asset::new_hashed(
                "paste",
                Kind::Js,
                include_bytes!("javascript/paste.js").to_vec(),
            ),
        }
    }
}
